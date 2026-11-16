<script setup lang="ts">
import { shallowRef, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError, saveTaskNote, useTaskNote } from '../../../entities/session';
import InspectorSection from './InspectorSection.vue';

const props = defineProps<{ worktreeId: string }>();

const note = useTaskNote(() => props.worktreeId);
const editing = shallowRef(false);
const draft = shallowRef('');
const busy = shallowRef(false);
const error = shallowRef<string | null>(null);

function edit() {
  draft.value = note.value;
  error.value = null;
  editing.value = true;
}

async function save(text: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await saveTaskNote(props.worktreeId, text);
    editing.value = false;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.worktreeId,
  () => {
    editing.value = false;
  },
);
</script>

<template>
  <InspectorSection :title="t('inspector.taskNote.title')" testid="inspector-task-note">
    <template v-if="!editing" #action>
      <UiButton type="button" class="link" data-testid="inspector-task-note-edit" @click="edit">{{ note ? t('inspector.taskNote.edit') : t('inspector.taskNote.add') }}</UiButton>
    </template>
    <form v-if="editing" class="form" data-testid="inspector-task-note-form" @submit.prevent="save(draft)" @keydown.esc.stop="editing = false">
      <UiTextarea v-model="draft" rows="4" :placeholder="t('taskNote.placeholder')" :aria-label="t('inspector.taskNote.title')" data-testid="inspector-task-note-text" autofocus />
      <p v-if="error" class="error" role="alert" data-testid="inspector-task-note-error">{{ error }}</p>
      <div class="actions">
        <UiButton type="button" class="secondary" :disabled="busy || !note" data-testid="inspector-task-note-clear" @click="save('')">{{ t('taskNote.clear') }}</UiButton>
        <UiButton type="button" class="secondary" @click="editing = false">{{ t('taskNote.cancel') }}</UiButton>
        <UiButton type="submit" class="primary" :disabled="busy" data-testid="inspector-task-note-save">{{ t('taskNote.save') }}</UiButton>
      </div>
    </form>
    <p v-else-if="note" class="text" data-testid="inspector-task-note-text-view">{{ note }}</p>
    <p v-else class="note">{{ t('inspector.taskNote.empty') }}</p>
  </InspectorSection>
</template>

<style scoped>
.text {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 8;
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.form :deep(textarea) {
  padding: 6px 8px;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  resize: vertical;
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border);
  border-radius: 6px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.primary,
.secondary {
  height: 26px;
  padding: 0 10px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  border: 0;
  border-radius: 7px;
  cursor: pointer;
}

.primary {
  color: var(--vr-color-accent-contrast);
  font-weight: 600;
  background: var(--vr-color-accent);
}

.secondary {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.secondary:disabled {
  color: var(--vr-color-text-subtle);
  cursor: default;
}
</style>
