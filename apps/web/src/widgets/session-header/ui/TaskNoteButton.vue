<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue';
import { FileText } from '../../../shared/ui/icons';
import { VButton, VMessage, VPopover, VTextarea, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError, saveTaskNote, useTaskNote } from '../../../entities/session';

const props = defineProps<{ worktreeId: string; branch: string }>();

const note = useTaskNote(() => props.worktreeId);
const open = ref(false);
const phone = useIsPhone();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const draft = ref('');
const busy = ref(false);
const error = ref<string | null>(null);

const label = computed(() => (note.value ? t('taskNote.openWithNote', { text: note.value }) : t('taskNote.open')));

function toggle(event: Event) {
  popover.value?.toggle(event);
}

function opened() {
  open.value = true;
  draft.value = note.value;
  error.value = null;
}

async function save(text: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await saveTaskNote(props.worktreeId, text);
    popover.value?.hide();
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.worktreeId,
  () => popover.value?.hide(),
);
</script>

<template>
  <div class="task-note">
    <VButton
      v-tooltip="label"
      :class="{ filled: !!note }"
      text
      severity="secondary"
      size="small"
      :label="phone ? t('taskNote.open') : undefined"
      :aria-label="label"
      :aria-expanded="open"
      aria-haspopup="dialog"
      data-testid="task-note-button"
      @click="toggle"
    >
      <template #icon><FileText :size="13" /></template>
    </VButton>
    <VPopover ref="popover" @show="opened" @hide="open = false">
      <form class="form" data-testid="task-note-form" @submit.prevent="save(draft)">
        <span class="heading">{{ t('taskNote.title', { branch }) }}</span>
        <VTextarea v-model="draft" rows="5" auto-resize :placeholder="t('taskNote.placeholder')" data-testid="task-note-text" autofocus />
        <span class="hint">{{ t('taskNote.hint') }}</span>
        <VMessage v-if="error" severity="error" size="small" data-testid="task-note-error">{{ error }}</VMessage>
        <div class="actions">
          <VButton :label="t('taskNote.clear')" text severity="secondary" size="small" :disabled="busy || !note" data-testid="task-note-clear" @click="save('')" />
          <VButton :label="t('taskNote.cancel')" text severity="secondary" size="small" @click="popover?.hide()" />
          <VButton :label="t('taskNote.save')" type="submit" size="small" :disabled="busy" data-testid="task-note-save" />
        </div>
      </form>
    </VPopover>
  </div>
</template>

<style scoped>
.task-note {
  display: flex;
  flex: none;
}

.filled {
  color: var(--vr-color-accent);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: min(340px, 80vw);
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
