<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from 'vue';
import { VButton, VDialog, VInputText, VMessage, VSelect, VTextarea, useIsPhone } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp, X } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSettings } from '../../../shared/config/settings/store';
import { chordJoiner, modifierGlyphs, onlyPrimaryPressed, primaryModifier, primaryPressed } from '../../../shared/lib/platform';
import { invoke } from '../../../bridge';
import { acpPrompt, createAcpAgent, createAgent, createTerminal, entitySessionRef, resumeConversation, type AgentKind, type SessionRef, useAction, useBerth } from '../../../entities/session';
import { enabledNativeAgent, useProviders, type Provider } from '../../../shared/api/providers';
import { useSessionTarget } from '../model/sessionTarget';
import { useKindStates, type KindState } from '../model/kindStates';
import { useProjectConversations } from '../model/conversations';
import { deletePreset, presets, savePreset, withPromptFrame, type AgentPreset } from '../model/presets';
import NewSessionTarget from './NewSessionTarget.vue';
import ContinueConversation from './ContinueConversation.vue';
import ChipField from './ChipField.vue';

const props = defineProps<{ worktree: string | null; projectLocked: boolean }>();
const emit = defineEmits<{ close: []; created: [session: SessionRef] }>();
const location = useSessionTarget(props.worktree);
const continuing = shallowRef(false);
const pickedConversation = shallowRef<string | null>(null);
const conversations = useProjectConversations(location.project, continuing);
const chosenConversation = computed(() => conversations.choices.value.find((choice) => choice.key === pickedConversation.value && !choice.busy) ?? null);

type AcpKind = `acp:${string}`;
type Kind = Exclude<AgentKind, 'acp'> | AcpKind | 'terminal';
const ON_BEFORE_PROVIDERS_LOAD: { label: string; value: Kind }[] = [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Grok', value: 'grok' },
];
const providers = useProviders();
const kindStates = useKindStates();
type KindOption = { label: string; value: Kind; state: KindState; disabled: boolean; provider: Provider | null };
const KINDS = computed<KindOption[]>(() => {
  const enabled = providers.value.filter((provider) => provider.enabled);
  const agents: KindOption[] = providers.value.length
    ? enabled.map((provider) => {
        const state = kindStates.stateOf(provider);
        const label = provider.acp ? t('agents.newSession.acpProvider', { name: provider.name }) : provider.name;
        return { label, value: (provider.acp ? `acp:${provider.id}` : provider.id) as Kind, state, disabled: state !== 'ready', provider };
      })
    : ON_BEFORE_PROVIDERS_LOAD.map((option) => ({ ...option, state: 'ready' as const, disabled: false, provider: null }));
  return [...agents, { label: t('agents.newSession.terminal'), value: 'terminal', state: 'ready', disabled: false, provider: null }];
});

const acpProviderOf = (kind: Kind): string | null => (kind.startsWith('acp:') ? kind.slice('acp:'.length) : null);

function generatedName(kind: Kind): string {
  const base = acpProviderOf(kind) ?? kind;
  const taken = location.takenNames.value;
  let n = 1;
  while (taken.has(`${base}-${n}`)) n++;
  return `${base}-${n}`;
}

const phone = useIsPhone();
const dialogPt = computed(() =>
  phone.value
    ? { mask: { style: { background: 'var(--vr-overlay)' } }, root: { style: { boxShadow: 'none' } }, content: { style: { display: 'flex', flexDirection: 'column', overflow: 'hidden' } } }
    : { mask: { style: { background: 'var(--vr-overlay)', alignItems: 'flex-start', paddingTop: '12vh' } }, root: { style: { borderRadius: '14px', boxShadow: 'var(--vr-shadow-dialog)' } } },
);

const { values } = useSettings();
const defaultKind = computed<Kind>(() => {
  const chosen = values.value['sessions.default_kind'];
  if (chosen === 'terminal') return 'terminal';
  if (!providers.value.length) return ON_BEFORE_PROVIDERS_LOAD.find((option) => option.value === chosen)?.value ?? 'claude';
  const preferred = (enabledNativeAgent(chosen, providers.value) as Kind | null) ?? 'terminal';
  const usable = KINDS.value.find((option) => option.value === preferred && !option.disabled) ?? KINDS.value.find((option) => !option.disabled);
  return usable?.value ?? 'terminal';
});
const picked = shallowRef<Kind | null>(null);
const kind = computed<Kind>(() => picked.value ?? defaultKind.value);
const name = shallowRef(generatedName(kind.value));
const model = shallowRef<string | null>(values.value['sessions.default_model']);
const effort = shallowRef<string | null>(values.value['sessions.default_effort']);
const prompt = shallowRef('');
const before = shallowRef('');
const after = shallowRef('');
const activePreset = shallowRef<string | null>(null);
const presetName = shallowRef('');
const action = useAction();
const started = shallowRef<{ text: string; session: SessionRef } | null>(null);
const berth = useBerth();
const daemonDown = computed(() => berth?.connection.value.state === 'offline');
const noProjects = computed(() => berth?.connection.value.state === 'connected' && !location.projects.value.length);
const expanded = shallowRef(false);
const backgroundKey = `${modifierGlyphs[primaryModifier]}${chordJoiner}Enter`;
const MODEL_SUGGESTIONS = ['opus', 'sonnet', 'haiku'];
const EFFORT_SUGGESTIONS = ['low', 'medium', 'high'];
const modelOptions = computed(() => (kind.value === 'claude' ? MODEL_SUGGESTIONS : []));

const defaultName = computed(() => generatedName(kind.value));

watch(defaultName, (next, previous) => {
  if (name.value.trim() === previous || !name.value.trim()) name.value = next;
});

const extras = shallowRef<Kind[]>([]);
const kinds = computed<Kind[]>(() => (kind.value === 'terminal' || continuing.value ? [kind.value] : [kind.value, ...extras.value.filter((extra) => extra !== kind.value)]));
const extraOptions = computed(() => KINDS.value.filter((option) => option.value !== 'terminal' && option.value !== kind.value && !option.disabled));
const many = computed(() => kinds.value.length > 1);

watch(
  kinds,
  (list) => {
    location.count.value = list.length;
  },
  { immediate: true },
);

function toggleExtra(value: Kind) {
  extras.value = extras.value.includes(value) ? extras.value.filter((extra) => extra !== value) : [...extras.value, value];
}

function pickKind(value: unknown) {
  const next = KINDS.value.find((option) => option.value === value);
  if (!next || next.disabled) return;
  if (name.value.trim() === defaultName.value || !name.value.trim()) name.value = generatedName(next.value);
  picked.value = next.value;
}

function applyPreset(preset: AgentPreset) {
  if (activePreset.value === preset.name) {
    activePreset.value = null;
    return;
  }
  activePreset.value = preset.name;
  pickKind(preset.kind);
  model.value = preset.model;
  effort.value = preset.effort;
  before.value = preset.before;
  after.value = preset.after;
}

function saveCurrent() {
  const trimmed = presetName.value.trim();
  if (!trimmed || kind.value === 'terminal') return;
  const acp = acpProviderOf(kind.value) !== null;
  savePreset({ name: trimmed, kind: kind.value, model: acp ? '' : (optional(model.value) ?? ''), effort: acp ? '' : (optional(effort.value) ?? ''), before: before.value, after: after.value });
  activePreset.value = trimmed;
  presetName.value = '';
}

function removePreset(name: string) {
  deletePreset(name);
  if (activePreset.value === name) activePreset.value = null;
}

const optional = (text: string | null) => text?.trim() || null;

const warmWorktree = computed(() => (location.plan.value.kind === 'use' && providers.value.length ? location.plan.value.worktree.id : null));

const warmTarget = computed(() => {
  const worktree = warmWorktree.value;
  const chosen = kind.value;
  if (!worktree || continuing.value || chosen === 'terminal' || acpProviderOf(chosen)) return null;
  return { worktree, kind: chosen, model: optional(model.value), effort: optional(effort.value) };
});

let prewarmTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => JSON.stringify(warmTarget.value),
  () => {
    clearTimeout(prewarmTimer);
    const target = warmTarget.value;
    if (!target) return;
    prewarmTimer = setTimeout(() => invoke<void>('berth_prewarm', target).catch(() => undefined), 150);
  },
  { immediate: true },
);
onBeforeUnmount(() => clearTimeout(prewarmTimer));

async function continueConversation(background: boolean) {
  const conversation = chosenConversation.value;
  if (!conversation) return;
  await action.run(async () => {
    const [worktree] = await location.resolve(['']);
    if (!worktree) return;
    const agent = await resumeConversation(conversation, worktree);
    if (background) startedInBackground(conversation.title, { Agent: agent }, 1);
    else emit('created', { Agent: agent });
  });
}

function startedInBackground(label: string, session: SessionRef, count: number) {
  started.value = { text: count > 1 ? t('agents.newSession.startedMany', { count }) : t('agents.newSession.started', { name: label }), session };
  prompt.value = '';
  if (!continuing.value) name.value = defaultName.value;
  pickedConversation.value = null;
  if (location.creating.value) location.fresh.value = '';
}

async function submit(background = false) {
  started.value = null;
  if (continuing.value) return continueConversation(background);
  const chosen = kinds.value;
  const defaults = chosen.map(generatedName);
  const names = chosen.length > 1 ? defaults : [name.value.trim() || defaults[0]];
  await action.run(async () => {
    const worktrees = await location.resolve(chosen.length > 1 ? chosen : ['']);
    let first: SessionRef | null = null;
    for (const [index, chosenKind] of chosen.entries()) {
      const sessionName = names[index];
      const worktree = worktrees[index];
      if (!sessionName || !worktree) continue;
      const acpProvider = acpProviderOf(chosenKind);
      const startingPrompt = withPromptFrame({ before: before.value, after: after.value }, prompt.value);
      const id =
        chosenKind === 'terminal'
          ? await createTerminal(worktree, sessionName)
          : acpProvider
            ? await createAcpAgent(worktree, sessionName, acpProvider, sessionName === defaults[index])
            : await createAgent({
              worktree,
              name: sessionName,
              kind: chosenKind as AgentKind,
              model: optional(model.value),
              effort: optional(effort.value),
              autoTitle: sessionName === defaults[index],
              startingPrompt,
            });
      const created = entitySessionRef(id);
      if (acpProvider && startingPrompt && created && 'Agent' in created) await acpPrompt(created.Agent, startingPrompt);
      first ??= created;
    }
    if (first && background) startedInBackground(names[0] ?? '', first, chosen.length);
    else if (first) emit('created', first);
    else emit('close');
  });
}

function startable(): boolean {
  return location.plan.value.kind !== 'incomplete' && !action.busy.value && (!continuing.value || !!chosenConversation.value);
}

function onPromptKey(event: KeyboardEvent) {
  if (phone.value || event.key !== 'Enter' || event.shiftKey || event.isComposing || primaryPressed(event)) return;
  event.preventDefault();
  if (startable()) void submit();
}

function onBranchEnter() {
  if (startable()) void submit();
}

function onFormKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.isComposing || !onlyPrimaryPressed(event)) return;
  event.preventDefault();
  if (startable()) void submit(true);
}

function onVisible(visible: boolean) {
  if (!visible) emit('close');
}
</script>

<template>
  <VDialog
    :visible="true"
    modal
    position="top"
    :closable="false"
    :aria-label="t('agents.newSession.title')"
    :style="{ width: '720px', maxWidth: '95vw' }"
    :pt="dialogPt"
    :draggable="false"
    @update:visible="onVisible"
  >
    <template #header>
      <span class="p-dialog-title">{{ t('agents.newSession.title') }}</span>
      <UiButton v-if="phone" type="button" class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="new-session-close" @click="emit('close')"><X :size="18" /></UiButton>
      <UiButton v-else type="button" class="esc" :title="t('common.close')" :aria-label="t('common.close')" data-testid="new-session-close" @click="emit('close')">Esc</UiButton>
    </template>
    <div v-if="noProjects" class="empty" data-testid="new-session-no-projects">
      <span class="empty-title">{{ t('agents.newSession.noProjectsTitle') }}</span>
      <span class="empty-text">{{ t('agents.newSession.noProjectsText') }}</span>
    </div>
    <form v-else class="form" data-testid="new-session-form" @submit.prevent="submit()" @keydown="onFormKey">
      <div class="body">
        <ContinueConversation v-if="continuing" v-model="pickedConversation" :conversations="conversations" />
        <VTextarea
          v-else-if="kind !== 'terminal'"
          v-model="prompt"
          rows="3"
          auto-resize
          class="prompt"
          :autofocus="!phone"
          :aria-label="t('agents.newSession.prompt')"
          :placeholder="phone ? t('agents.newSession.promptPlaceholderPhone') : t('agents.newSession.promptPlaceholder')"
          data-testid="new-session-prompt"
          @keydown="onPromptKey"
        />
        <NewSessionTarget :state="location" :project-locked="projectLocked" :resume="continuing" @enter="onBranchEnter">
          <template #type>
            <ChipField v-if="!continuing" :label="t('agents.newSession.agent')" data-testid="new-session-kind-field">
              <template #default="{ labelId, controlId }">
                <VSelect
                  :model-value="kind"
                  :options="KINDS"
                  option-label="label"
                  option-value="value"
                  option-disabled="disabled"
                  size="small"
                  class="chip"
                  :label-id="controlId"
                  :aria-labelledby="labelId"
                  data-testid="new-session-kind"
                  @update:model-value="pickKind"
                >
                  <template #option="{ option }">
                    <span class="kind-option" :data-state="option.state" :data-testid="`new-session-kind-${option.value}`">
                      <span class="kind-label">
                        <span>{{ option.label }}</span>
                        <span v-if="option.value === 'terminal'" class="kind-sub">{{ t('agents.newSession.terminalHint') }}</span>
                      </span>
                      <span v-if="option.state === 'missing'" class="kind-badge">{{ t('agents.newSession.kindMissing') }}</span>
                      <template v-else-if="option.state === 'signedOut'">
                        <span class="kind-badge">{{ t('agents.newSession.kindSignedOut') }}</span>
                        <UiButton v-if="option.provider" type="button" class="kind-action" data-testid="new-session-kind-sign-in" @click.stop="kindStates.signIn(option.provider)">{{ t('agents.newSession.kindSignIn') }}</UiButton>
                      </template>
                    </span>
                  </template>
                </VSelect>
              </template>
            </ChipField>
          </template>
        </NewSessionTarget>
        <div class="chips" data-testid="new-session-options">
          <template v-if="kind !== 'terminal' && !acpProviderOf(kind) && !continuing">
            <ChipField class="field-input" :label="t('agents.newSession.model')" data-testid="new-session-model-field">
              <template #default="{ labelId, controlId }">
                <VSelect
                  v-model="model"
                  :options="modelOptions"
                  editable
                  size="small"
                  class="chip chip-input"
                  :placeholder="t('agents.newSession.defaultModel')"
                  :label-id="controlId"
                  :aria-labelledby="labelId"
                  data-testid="new-session-model"
                />
              </template>
            </ChipField>
            <ChipField class="field-input" :label="t('agents.newSession.effort')" data-testid="new-session-effort-field">
              <template #default="{ labelId, controlId }">
                <VSelect
                  v-model="effort"
                  :options="EFFORT_SUGGESTIONS"
                  editable
                  size="small"
                  class="chip chip-input"
                  :placeholder="t('agents.newSession.defaultEffort')"
                  :label-id="controlId"
                  :aria-labelledby="labelId"
                  data-testid="new-session-effort"
                />
              </template>
            </ChipField>
          </template>
          <span class="spacer" />
          <UiButton type="button" class="chips-action" :aria-pressed="continuing" data-testid="new-session-continue" @click="continuing = !continuing">
            <span>{{ continuing ? t('agents.newSession.newTask') : t('agents.newSession.continueMode') }}</span>
            <component :is="continuing ? ChevronLeft : ChevronRight" :size="14" aria-hidden="true" />
          </UiButton>
          <UiButton type="button" class="chips-action" :aria-expanded="expanded" data-testid="new-session-more" @click="expanded = !expanded">
            <span>{{ expanded ? t('agents.newSession.fewer') : t('agents.newSession.more') }}</span>
            <component :is="expanded ? ChevronUp : ChevronDown" :size="14" aria-hidden="true" />
          </UiButton>
        </div>
        <label v-if="expanded && location.plan.value.kind === 'create'" class="row">
          <span class="row-label">{{ t('agents.newSession.setupCommand') }}</span>
          <VInputText v-model="location.setup.value" size="small" :placeholder="t('agents.newSession.setupPlaceholder')" class="row-input mono" data-testid="new-session-setup" />
        </label>
        <div v-if="expanded && kind !== 'terminal' && !continuing" class="extras" data-testid="new-session-extras">
          <span class="extras-label">{{ t('agents.newSession.alsoStart') }}</span>
          <VButton
            v-for="option in extraOptions"
            :key="option.value"
            :label="option.label"
            size="small"
            :outlined="!extras.includes(option.value)"
            :aria-pressed="extras.includes(option.value)"
            :data-testid="`new-session-extra-${option.value}`"
            @click="toggleExtra(option.value)"
          />
        </div>
        <div v-if="expanded && presets.length && !continuing" class="extras" data-testid="new-session-presets">
          <span class="extras-label">{{ t('agents.newSession.presets') }}</span>
          <span v-for="preset in presets" :key="preset.name" class="preset">
            <VButton
              :label="preset.name"
              size="small"
              :outlined="activePreset !== preset.name"
              :aria-pressed="activePreset === preset.name"
              :data-testid="`new-session-preset-${preset.name}`"
              @click="applyPreset(preset)"
            />
            <VButton
              icon="pi pi-times"
              size="small"
              text
              :aria-label="t('agents.newSession.deletePreset', { name: preset.name })"
              :data-testid="`new-session-preset-delete-${preset.name}`"
              @click="removePreset(preset.name)"
            />
          </span>
        </div>
        <div v-if="expanded && kind !== 'terminal' && !continuing" class="pair">
          <label class="field">
            <span>{{ t('agents.newSession.before') }}</span>
            <VInputText v-model="before" size="small" data-testid="new-session-before" />
          </label>
          <label class="field">
            <span>{{ t('agents.newSession.after') }}</span>
            <VInputText v-model="after" size="small" data-testid="new-session-after" />
          </label>
        </div>
        <div v-if="expanded && kind !== 'terminal' && !continuing" class="extras">
          <VInputText v-model="presetName" size="small" :placeholder="t('agents.newSession.presetName')" data-testid="new-session-preset-name" @keydown.enter.prevent="saveCurrent" />
          <VButton :label="t('agents.newSession.savePreset')" size="small" outlined :disabled="!presetName.trim()" data-testid="new-session-preset-save" @click="saveCurrent" />
        </div>
      </div>
      <VMessage v-if="daemonDown" severity="error" size="small" data-testid="new-session-daemon-down">{{ t('agents.notices.offline') }}</VMessage>
      <VMessage v-else-if="action.error.value" severity="error" size="small" data-testid="new-session-error">{{ action.error.value }}</VMessage>
      <p v-if="started" class="started" role="status" data-testid="new-session-started">
        <span>{{ started.text }}</span>
        <UiButton type="button" class="started-open" data-testid="new-session-started-open" @click="emit('created', started.session)">{{ t('agents.newSession.startedOpen') }}</UiButton>
      </p>
      <div class="footer">
        <label v-if="!many && !continuing" class="name">
          <span>{{ t('agents.newSession.name') }}</span>
          <VInputText v-model="name" size="small" :placeholder="defaultName" data-testid="new-session-name" />
        </label>
        <span class="spacer" />
        <span v-if="!phone" class="hint" data-testid="new-session-background-hint">{{ t('agents.newSession.backgroundHint', { key: backgroundKey }) }}</span>
        <VButton class="action" :label="t('common.cancel')" :size="phone ? undefined : 'small'" text data-testid="new-session-cancel" @click="emit('close')" />
        <VButton class="action" type="submit" :label="continuing ? t('agents.newSession.continueStart') : many ? t('agents.newSession.startMany', { count: kinds.length }) : t('common.create')" :size="phone ? undefined : 'small'" :loading="action.busy.value" :disabled="location.plan.value.kind === 'incomplete' || (continuing && !chosenConversation)" data-testid="new-session-submit" />
      </div>
    </form>
  </VDialog>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.chip-input {
  width: 10.5rem;
}

.chips-action {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 28px;
  padding: 0 var(--vr-space-2) 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  white-space: nowrap;
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.chips-action:hover {
  background: var(--vr-color-nav-hover);
}

.chips-action svg {
  flex: none;
  color: var(--vr-color-text-muted);
}

.esc {
  margin-left: auto;
  padding: 1px 6px;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 4px;
}

.esc:hover {
  color: var(--vr-color-text);
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.row-label {
  width: 7.5rem;
  flex: none;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-input {
  flex: 1;
  min-width: 0;
}

.form :deep(.p-inputtext) {
  border-radius: var(--vr-radius-sm);
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-label);
}

.hint {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.empty {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) 0;
}

.empty-title {
  font-size: var(--vr-font-size-title);
  font-weight: 600;
}

.empty-text {
  color: var(--vr-color-text-muted);
}

.started {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  margin: 0;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-muted);
}

.started-open {
  padding: 0;
  font: inherit;
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}

.kind-option {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  width: 100%;
  min-width: 12rem;
}

.kind-option[data-state='signedOut'] {
  opacity: 0.75;
}

.kind-option[data-state='missing'] {
  opacity: 0.55;
}

.kind-label {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.kind-sub {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.kind-badge {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
}

.kind-action {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-link);
  white-space: nowrap;
  cursor: pointer;
  background: none;
  border: 0;
  pointer-events: auto;
}

.prompt {
  min-height: 84px;
  padding: 12px 14px;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 12px;
  background: var(--vr-color-surface-sunken);
  box-shadow: none;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  resize: none;
}

.chip {
  height: 28px;
  min-width: 0;
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
  box-shadow: none;
  font-size: var(--vr-font-size-label);
}

.chip :deep(.p-select-label) {
  padding: 0 9px;
  white-space: nowrap;
}

.chip :deep(.p-select-dropdown) {
  width: 1.6rem;
}

.pair {
  display: flex;
  gap: var(--vr-space-3);
}

.extras {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.extras-label {
  margin-right: 4px;
}

.preset {
  display: inline-flex;
  align-items: center;
}

.field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.footer {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: var(--vr-space-2);
  border-top: 1px solid var(--vr-color-border);
}

.name {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.name :deep(input) {
  width: 150px;
}

.spacer {
  flex: 1;
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--vr-size-icon-touch);
  height: var(--vr-size-icon-touch);
  margin-left: auto;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

:root[data-vr-viewport='phone'] .form {
  flex: 1 1 auto;
  min-height: 0;
}

:root[data-vr-viewport='phone'] .body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  scroll-padding-block: var(--vr-space-8);
}

:root[data-vr-viewport='phone'] .body > * {
  flex: none;
}

:root[data-vr-viewport='phone'] .chips {
  display: grid;
  grid-template-columns: 1fr 1fr;
}

:root[data-vr-viewport='phone'] .field-input {
  grid-column: 1 / -1;
}

:root[data-vr-viewport='phone'] .chip-input {
  width: 100%;
}

:root[data-vr-viewport='phone'] .chip {
  height: auto;
  min-height: var(--vr-size-row-touch);
}

:root[data-vr-viewport='phone'] .chip:not(.chip-input) :deep(.p-select-label) {
  display: flex;
  align-items: center;
  align-self: stretch;
  min-height: var(--vr-size-row-touch);
}

:root[data-vr-viewport='phone'] .chips .spacer {
  display: none;
}

:root[data-vr-viewport='phone'] .chips-action {
  grid-column: 1 / -1;
  justify-content: space-between;
  min-height: var(--vr-size-row-touch);
  padding: 0 var(--vr-space-3);
}

:root[data-vr-viewport='phone'] .row,
:root[data-vr-viewport='phone'] .pair {
  flex-direction: column;
  align-items: stretch;
  gap: var(--vr-space-1);
}

:root[data-vr-viewport='phone'] .row-label {
  width: auto;
}

:root[data-vr-viewport='phone'] .footer {
  flex: none;
  flex-wrap: wrap;
  padding-bottom: var(--vr-space-2);
}

:root[data-vr-viewport='phone'] .footer .spacer {
  display: none;
}

:root[data-vr-viewport='phone'] .name {
  flex: 1 0 100%;
  flex-direction: column;
  align-items: stretch;
  gap: var(--vr-space-1);
}

:root[data-vr-viewport='phone'] .name :deep(input) {
  width: 100%;
}

:root[data-vr-viewport='phone'] .action {
  flex: 1 1 0;
  min-height: var(--vr-size-row-touch);
}
</style>
