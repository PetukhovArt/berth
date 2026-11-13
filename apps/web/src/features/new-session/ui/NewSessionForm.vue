<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { VButton, VDialog, VInputText, VMessage, VSelect, VTextarea } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSettings } from '../../../shared/config/settings/store';
import { createAgent, createTerminal, entitySessionRef, type AgentKind, type SessionRef, useAction } from '../../../entities/session';
import { useSessionTarget } from '../model/sessionTarget';
import NewSessionTarget from './NewSessionTarget.vue';

const props = defineProps<{ worktree: string | null }>();
const emit = defineEmits<{ close: []; created: [session: SessionRef] }>();
const location = useSessionTarget(props.worktree);

type Kind = AgentKind | 'terminal';
const KINDS = computed<{ label: string; value: Kind }[]>(() => [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Cursor', value: 'cursor' },
  { label: 'OpenCode', value: 'opencode' },
  { label: t('agents.newSession.terminal'), value: 'terminal' },
]);

function generatedName(kind: Kind): string {
  const taken = location.takenNames.value;
  let n = 1;
  while (taken.has(`${kind}-${n}`)) n++;
  return `${kind}-${n}`;
}

const { values } = useSettings();
const initialKind = KINDS.value.find((option) => option.value === values.value['sessions.default_kind'])?.value ?? 'claude';
const kind = shallowRef<Kind>(initialKind);
const name = shallowRef(generatedName(initialKind));
const model = shallowRef(values.value['sessions.default_model']);
const effort = shallowRef(values.value['sessions.default_effort']);
const prompt = shallowRef('');
const action = useAction();

const defaultName = computed(() => generatedName(kind.value));

watch(defaultName, (next, previous) => {
  if (name.value.trim() === previous || !name.value.trim()) name.value = next;
});

function pickKind(value: unknown) {
  const next = KINDS.value.find((option) => option.value === value);
  if (!next) return;
  if (name.value.trim() === defaultName.value || !name.value.trim()) name.value = generatedName(next.value);
  kind.value = next.value;
}

const optional = (text: string) => text.trim() || null;

async function submit() {
  const chosen = kind.value;
  const finalName = name.value.trim() || defaultName.value;
  await action.run(async () => {
    const worktree = await location.resolve();
    const id =
      chosen === 'terminal'
        ? await createTerminal(worktree, finalName)
        : await createAgent({
            worktree,
            name: finalName,
            kind: chosen,
            model: optional(model.value),
            effort: optional(effort.value),
            autoTitle: finalName === defaultName.value,
            startingPrompt: optional(prompt.value),
          });
    const created = entitySessionRef(id);
    if (created) emit('created', created);
    else emit('close');
  });
}

function onPromptKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  if (location.plan.value.kind === 'incomplete' || action.busy.value) return;
  void submit();
}

function onVisible(visible: boolean) {
  if (!visible) emit('close');
}
</script>

<template>
  <VDialog
    :visible="true"
    modal
    :header="t('agents.newSession.title')"
    :style="{ width: '640px', maxWidth: '95vw' }"
    :pt="{ mask: { style: { background: 'var(--vr-overlay)' } }, root: { style: { borderRadius: '14px', boxShadow: 'var(--vr-shadow-dialog)' } } }"
    :draggable="false"
    @update:visible="onVisible"
  >
    <form class="form" data-testid="new-session-form" @submit.prevent="submit">
      <VTextarea
        v-if="kind !== 'terminal'"
        v-model="prompt"
        rows="4"
        auto-resize
        class="prompt"
        :aria-label="t('agents.newSession.prompt')"
        :placeholder="t('agents.newSession.optional')"
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
      <div v-if="kind !== 'terminal'" class="pair">
        <label class="field">
          <span>{{ t('agents.newSession.model') }}</span>
          <VInputText v-model="model" size="small" :placeholder="t('agents.newSession.byDefault')" data-testid="new-session-model" />
        </label>
        <label class="field">
          <span>{{ t('agents.newSession.effort') }}</span>
          <VInputText v-model="effort" size="small" :placeholder="t('agents.newSession.byDefault')" data-testid="new-session-effort" />
        </label>
      </div>
      <VMessage v-if="action.error.value" severity="error" size="small" data-testid="new-session-error">{{ action.error.value }}</VMessage>
      <div class="footer">
        <label class="name">
          <span>{{ t('agents.newSession.name') }}</span>
          <VInputText v-model="name" size="small" :placeholder="defaultName" data-testid="new-session-name" />
        </label>
        <span class="spacer" />
        <VButton :label="t('common.cancel')" size="small" text data-testid="new-session-cancel" @click="emit('close')" />
        <VButton type="submit" :label="t('common.create')" size="small" :loading="action.busy.value" :disabled="location.plan.value.kind === 'incomplete'" data-testid="new-session-submit" />
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

.prompt {
  min-height: 120px;
  padding: 12px 14px;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 12px;
  background: var(--vr-color-surface-sunken);
  box-shadow: none;
  font-size: 14px;
  line-height: 22px;
  resize: none;
}

.chip {
  height: 28px;
  min-width: 0;
  border: 0;
  border-radius: 7px;
  background: var(--vr-color-surface-muted);
  box-shadow: none;
  font-size: 12.5px;
}

.chip :deep(.p-select-label) {
  padding: 0 9px;
  white-space: nowrap;
}

.pair {
  display: flex;
  gap: var(--vr-space-3);
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
  font-size: 12px;
  color: var(--vr-color-text-muted);
}

.name :deep(input) {
  width: 150px;
}

.spacer {
  flex: 1;
}
</style>
