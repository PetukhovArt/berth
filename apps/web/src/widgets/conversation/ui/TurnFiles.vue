<script setup lang="ts">
import { computed } from 'vue';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { worktreeRelative } from '../../../shared/lib/worktreePath';
import { isAvailable, showChangeIn } from '../../../features/session-view-switch';
import type { ChangedFile } from '../model/chatBlocks';

const props = defineProps<{ files: ChangedFile[]; root: string | null; panelId?: string }>();

const canOpen = computed(() => !!props.panelId && isAvailable(props.panelId, 'diff'));

function open(file: ChangedFile) {
  if (props.panelId) showChangeIn(props.panelId, file.path);
}
</script>

<template>
  <section class="card" data-testid="chat-changed-files">
    <header class="head">
      <span class="title">{{ t('chat.changedFiles') }}</span>
      <span class="count">{{ t('chat.fileCount', { count: files.length }) }}</span>
    </header>
    <ul class="files">
      <li v-for="file in files" :key="file.path" class="file" data-testid="chat-changed-file">
        <span v-tooltip="file.path" class="path mono">{{ worktreeRelative(file.path, root) }}</span>
        <span v-if="file.additions" class="added mono">+{{ file.additions }}</span>
        <span v-if="file.deletions" class="removed mono">-{{ file.deletions }}</span>
        <VButton v-if="canOpen" class="open" :label="t('chat.openInChanges')" text size="small" data-testid="chat-changed-file-open" @click="open(file)" />
      </li>
    </ul>
  </section>
</template>

<style scoped>
.card {
  overflow: hidden;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 34px;
  padding: 0 var(--vr-space-3);
  font-size: var(--vr-font-size-label);
  background: var(--vr-color-surface);
}

.title {
  font-weight: 500;
}

.count {
  color: var(--vr-color-text-muted);
}

.files {
  margin: 0;
  padding: 0;
  list-style: none;
}

.file {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 32px;
  padding: 0 var(--vr-space-3);
  font-size: var(--vr-font-size-label);
  border-top: 1px solid var(--vr-color-border);
}

.path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.added,
.removed {
  flex: none;
  font-size: var(--vr-font-size-2xs);
}

.added {
  color: var(--vr-color-success);
}

.removed {
  color: var(--vr-color-danger);
}

.open {
  flex: none;
  white-space: nowrap;
}

.mono {
  font-family: var(--vr-font-mono);
}
</style>
