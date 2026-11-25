<script setup lang="ts">
import { computed } from 'vue';
import { VButton, VDialog } from '../../../shared/ui/kit';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { fileNameOf } from '../../../shared/lib/markdownFiles';
import { unsavedPrompt } from '../../../entities/document';

const names = computed(() => (unsavedPrompt.value?.paths ?? []).map(fileNameOf).join(', '));

function onVisible(visible: boolean) {
  if (!visible) unsavedPrompt.value?.settle('cancel');
}
</script>

<template>
  <VDialog :visible="unsavedPrompt !== null" modal :header="t('files.editor.unsaved.title')" :style="{ width: '480px', maxWidth: '95vw' }" :draggable="false" @update:visible="onVisible">
    <p class="message" data-testid="unsaved-message">{{ t('files.editor.unsaved.message', { names }) }}</p>
    <DialogActions :confirm-label="t('common.save')" focus="cancel" cancel-testid="unsaved-cancel" confirm-testid="unsaved-save" @cancel="unsavedPrompt?.settle('cancel')" @confirm="unsavedPrompt?.settle('save')">
      <template #middle="{ size }">
        <VButton :label="t('files.editor.unsaved.discard')" :size="size" severity="danger" text data-testid="unsaved-discard" @click="unsavedPrompt?.settle('discard')" />
      </template>
    </DialogActions>
  </VDialog>
</template>

<style scoped>
.message {
  margin: 0 0 var(--vr-space-4);
  overflow-wrap: anywhere;
  color: var(--vr-color-text-muted);
}
</style>
