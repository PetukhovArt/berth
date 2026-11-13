<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { FileText } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError, saveTaskNote, useTaskNote } from '../../../entities/session';

const props = defineProps<{ worktreeId: string; branch: string }>();

const note = useTaskNote(() => props.worktreeId);
const open = ref(false);
const draft = ref('');
const busy = ref(false);
const error = ref<string | null>(null);

const label = computed(() => (note.value ? t('taskNote.openWithNote', { text: note.value }) : t('taskNote.open')));

function toggle() {
  open.value = !open.value;
  if (open.value) {
    draft.value = note.value;
    error.value = null;
  }
}

async function save(text: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await saveTaskNote(props.worktreeId, text);
    open.value = false;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.worktreeId,
  () => {
    open.value = false;
  },
);
</script>

<template>
  <div class="task-note">
    <UiButton type="button" class="chip" :class="{ filled: !!note }" :title="label" :aria-label="label" :aria-expanded="open" data-testid="task-note-button" @click="toggle">
      <FileText :size="13" />
    </UiButton>
    <form v-if="open" class="form vr-sheet" data-testid="task-note-form" @submit.prevent="save(draft)">
      <span class="heading">{{ t('taskNote.title', { branch }) }}</span>
      <UiTextarea v-model="draft" rows="5" :placeholder="t('taskNote.placeholder')" data-testid="task-note-text" autofocus />
      <span class="hint">{{ t('taskNote.hint') }}</span>
      <span v-if="error" class="error" role="alert" data-testid="task-note-error">{{ error }}</span>
      <div class="actions">
        <UiButton type="button" class="secondary" :disabled="busy || !note" data-testid="task-note-clear" @click="save('')">{{ t('taskNote.clear') }}</UiButton>
        <UiButton type="button" class="secondary" @click="open = false">{{ t('taskNote.cancel') }}</UiButton>
        <UiButton type="submit" class="primary" :disabled="busy" data-testid="task-note-save">{{ t('taskNote.save') }}</UiButton>
      </div>
    </form>
  </div>
</template>

<style scoped>
.task-note {
  position: relative;
  display: flex;
  flex: none;
}

.chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--vr-color-text-subtle);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-full);
  cursor: pointer;
}

.chip.filled {
  color: var(--vr-color-accent);
  background: var(--vr-color-surface-muted);
}

.form {
  position: absolute;
  top: 28px;
  right: 0;
  z-index: 6;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: 340px;
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-raised);
  border-radius: 9px;
  box-shadow: var(--vr-shadow-popover);
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.error {
  color: var(--vr-status-crashed);
  font-size: var(--vr-font-size-xs);
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
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.primary,
.secondary {
  height: 28px;
  padding: 0 11px;
  font: inherit;
  font-size: 12.5px;
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
