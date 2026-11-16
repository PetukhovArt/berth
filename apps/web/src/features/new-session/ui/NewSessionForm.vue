<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { VButton, VDialog, VInputText, VMessage, VSelect, VTextarea } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSettings } from '../../../shared/config/settings/store';
import { chordJoiner, modifierGlyphs, onlyPrimaryPressed, primaryModifier, primaryPressed } from '../../../shared/lib/platform';
import { invoke } from '../../../bridge';
import { acpPrompt, createAcpAgent, createAgent, createTerminal, entitySessionRef, type AgentKind, type SessionRef, useAction } from '../../../entities/session';
import { enabledNativeAgent, useProviders } from '../../../shared/api/providers';
import { useSessionTarget } from '../model/sessionTarget';
import { deletePreset, presets, savePreset, withPromptFrame, type AgentPreset } from '../model/presets';
import NewSessionTarget from './NewSessionTarget.vue';

const props = defineProps<{ worktree: string | null }>();
const emit = defineEmits<{ close: []; created: [session: SessionRef] }>();
const location = useSessionTarget(props.worktree);

type AcpKind = `acp:${string}`;
type Kind = Exclude<AgentKind, 'acp'> | AcpKind | 'terminal';
const ON_BEFORE_PROVIDERS_LOAD: { label: string; value: Kind }[] = [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Grok', value: 'grok' },
];
const providers = useProviders();
const KINDS = computed<{ label: string; value: Kind }[]>(() => {
  const enabled = providers.value.filter((provider) => provider.enabled);
  const agents: { label: string; value: Kind }[] = providers.value.length
    ? enabled.map((provider) => (provider.acp ? { label: t('agents.newSession.acpProvider', { name: provider.name }), value: `acp:${provider.id}` as const } : { label: provider.name, value: provider.id as Kind }))
    : ON_BEFORE_PROVIDERS_LOAD;
  return [...agents, { label: t('agents.newSession.terminal'), value: 'terminal' }];
});

const acpProviderOf = (kind: Kind): string | null => (kind.startsWith('acp:') ? kind.slice('acp:'.length) : null);

function generatedName(kind: Kind): string {
  const base = acpProviderOf(kind) ?? kind;
  const taken = location.takenNames.value;
  let n = 1;
  while (taken.has(`${base}-${n}`)) n++;
  return `${base}-${n}`;
}

const { values } = useSettings();
const defaultKind = computed<Kind>(() => {
  const chosen = values.value['sessions.default_kind'];
  if (chosen === 'terminal') return 'terminal';
  if (!providers.value.length) return ON_BEFORE_PROVIDERS_LOAD.find((option) => option.value === chosen)?.value ?? 'claude';
  return (enabledNativeAgent(chosen, providers.value) as Kind | null) ?? 'terminal';
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
const kinds = computed<Kind[]>(() => (kind.value === 'terminal' ? [kind.value] : [kind.value, ...extras.value.filter((extra) => extra !== kind.value)]));
const extraOptions = computed(() => KINDS.value.filter((option) => option.value !== 'terminal' && option.value !== kind.value));
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
  if (!next) return;
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

watch(
  [warmWorktree, kind],
  ([worktree, chosen]) => {
    if (!worktree || chosen === 'terminal' || acpProviderOf(chosen)) return;
    invoke<void>('berth_prewarm', { worktree, kind: chosen, model: optional(model.value), effort: optional(effort.value) }).catch(() => undefined);
  },
  { immediate: true },
);

async function submit(background = false) {
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
    if (first && !background) emit('created', first);
    else emit('close');
  });
}

function startable(): boolean {
  return location.plan.value.kind !== 'incomplete' && !action.busy.value;
}

function onPromptKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing || primaryPressed(event)) return;
  event.preventDefault();
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
    :pt="{ mask: { style: { background: 'var(--vr-overlay)', alignItems: 'flex-start', paddingTop: '12vh' } }, root: { style: { borderRadius: '14px', boxShadow: 'var(--vr-shadow-dialog)' } } }"
    :draggable="false"
    @update:visible="onVisible"
  >
    <template #header>
      <span class="p-dialog-title">{{ t('agents.newSession.title') }}</span>
      <UiButton type="button" class="esc" :title="t('common.close')" :aria-label="t('common.close')" data-testid="new-session-close" @click="emit('close')">Esc</UiButton>
    </template>
    <form class="form" data-testid="new-session-form" @submit.prevent="submit()" @keydown="onFormKey">
      <VTextarea
        v-if="kind !== 'terminal'"
        v-model="prompt"
        rows="3"
        auto-resize
        class="prompt"
        :aria-label="t('agents.newSession.prompt')"
        :placeholder="t('agents.newSession.promptPlaceholder')"
        data-testid="new-session-prompt"
        @keydown="onPromptKey"
      />
      <NewSessionTarget :state="location">
        <template #type>
          <VSelect
            :model-value="kind"
            :options="KINDS"
            option-label="label"
            option-value="value"
            size="small"
            class="chip"
            :aria-label="t('agents.newSession.typeAria')"
            data-testid="new-session-kind"
            @update:model-value="pickKind"
          />
        </template>
      </NewSessionTarget>
      <div class="chips" data-testid="new-session-options">
        <template v-if="kind !== 'terminal' && !acpProviderOf(kind)">
          <VSelect
            v-model="model"
            :options="modelOptions"
            editable
            size="small"
            class="chip chip-input"
            :placeholder="t('agents.newSession.defaultModel')"
            :aria-label="t('agents.newSession.model')"
            data-testid="new-session-model"
          />
          <VSelect
            v-model="effort"
            :options="EFFORT_SUGGESTIONS"
            editable
            size="small"
            class="chip chip-input"
            :placeholder="t('agents.newSession.defaultEffort')"
            :aria-label="t('agents.newSession.effort')"
            data-testid="new-session-effort"
          />
        </template>
        <span class="spacer" />
        <VButton
          :label="expanded ? t('agents.newSession.fewer') : t('agents.newSession.more')"
          size="small"
          text
          :aria-expanded="expanded"
          data-testid="new-session-more"
          @click="expanded = !expanded"
        />
      </div>
      <label v-if="expanded && location.plan.value.kind === 'create'" class="row">
        <span class="row-label">{{ t('agents.newSession.setupCommand') }}</span>
        <VInputText v-model="location.setup.value" size="small" :placeholder="t('agents.newSession.setupPlaceholder')" class="row-input mono" data-testid="new-session-setup" />
      </label>
      <div v-if="expanded && kind !== 'terminal'" class="extras" data-testid="new-session-extras">
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
      <div v-if="expanded && presets.length" class="extras" data-testid="new-session-presets">
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
      <div v-if="expanded && kind !== 'terminal'" class="pair">
        <label class="field">
          <span>{{ t('agents.newSession.before') }}</span>
          <VInputText v-model="before" size="small" data-testid="new-session-before" />
        </label>
        <label class="field">
          <span>{{ t('agents.newSession.after') }}</span>
          <VInputText v-model="after" size="small" data-testid="new-session-after" />
        </label>
      </div>
      <div v-if="expanded && kind !== 'terminal'" class="extras">
        <VInputText v-model="presetName" size="small" :placeholder="t('agents.newSession.presetName')" data-testid="new-session-preset-name" @keydown.enter.prevent="saveCurrent" />
        <VButton :label="t('agents.newSession.savePreset')" size="small" outlined :disabled="!presetName.trim()" data-testid="new-session-preset-save" @click="saveCurrent" />
      </div>
      <VMessage v-if="action.error.value" severity="error" size="small" data-testid="new-session-error">{{ action.error.value }}</VMessage>
      <div class="footer">
        <label v-if="!many" class="name">
          <span>{{ t('agents.newSession.name') }}</span>
          <VInputText v-model="name" size="small" :placeholder="defaultName" data-testid="new-session-name" />
        </label>
        <span class="spacer" />
        <span class="hint" data-testid="new-session-background-hint">{{ t('agents.newSession.backgroundHint', { key: backgroundKey }) }}</span>
        <VButton :label="t('common.cancel')" size="small" text data-testid="new-session-cancel" @click="emit('close')" />
        <VButton type="submit" :label="many ? t('agents.newSession.startMany', { count: kinds.length }) : t('common.create')" size="small" :loading="action.busy.value" :disabled="location.plan.value.kind === 'incomplete'" data-testid="new-session-submit" />
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

.chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.chip-input {
  width: 10.5rem;
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

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-label);
}

.hint {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
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
  border-radius: 7px;
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
</style>
