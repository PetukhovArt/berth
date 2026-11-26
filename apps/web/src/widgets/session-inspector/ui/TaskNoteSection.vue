<script setup lang="ts">
import { shallowRef, watch } from 'vue';
import { VButton, VMessage, VTextarea } from '../../../shared/ui/kit';
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
      <VButton :label="note ? t('inspector.taskNote.edit') : t('inspector.taskNote.add')" text size="small" data-testid="inspector-task-note-edit" @click="edit" />
    </template>
    <form v-if="editing" class="form" data-testid="inspector-task-note-form" @submit.prevent="save(draft)" @keydown.esc.stop="editing = false">
      <VTextarea v-model="draft" rows="4" auto-resize :placeholder="t('taskNote.placeholder')" :aria-label="t('inspector.taskNote.title')" data-testid="inspector-task-note-text" autofocus />
      <VMessage v-if="error" severity="error" size="small" data-testid="inspector-task-note-error">{{ error }}</VMessage>
      <div class="actions">
        <VButton :label="t('taskNote.clear')" text severity="secondary" size="small" :disabled="busy || !note" data-testid="inspector-task-note-clear" @click="save('')" />
        <VButton :label="t('taskNote.cancel')" text severity="secondary" size="small" @click="editing = false" />
        <VButton type="submit" :label="t('taskNote.save')" size="small" :disabled="busy" data-testid="inspector-task-note-save" />
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

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
