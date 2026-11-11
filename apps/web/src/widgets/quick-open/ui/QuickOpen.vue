<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { listWorktreeMarkdown, type MarkdownEntry } from '../../../shared/lib/markdownFiles';
import QuickSwitcher from './QuickSwitcher.vue';
import { isDesktop } from '../../../bridge';
import { openCleanup } from '../../../features/cleanup';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';
import { useSessionDock } from '../../../entities/session';
import { openLeaf, quickItems, type WorkNode, useWorkTree } from '../../../features/work-tree';

const sessions = useSessionDock();
const { all } = useWorkTree();
const { switching } = useDock();
const { activeWorktree } = sessions;
const entries = computed(() => (switching.value ? quickItems(all.value) : []));
const files = shallowRef<MarkdownEntry[]>([]);
const actions = computed(() => (isDesktop ? [{ key: 'cleanup', label: t('cleanup.action') }] : []));

watch(switching, async (on) => {
  files.value = [];
  const worktree = activeWorktree.value;
  if (!on || !worktree) return;
  try {
    const found = await listWorktreeMarkdown(worktree.path);
    if (switching.value) files.value = found;
  } catch {
    files.value = [];
  }
});

function pick(node: WorkNode) {
  switching.value = false;
  openLeaf(node);
}

function pickAction(key: string) {
  switching.value = false;
  if (key === 'cleanup') openCleanup();
}

function pickFile(path: string) {
  switching.value = false;
  sessions.openFile(path, { worktreeId: activeWorktree.value?.id });
}
</script>

<template>
  <Transition name="qs">
    <QuickSwitcher v-if="switching" :entries="entries" :files="files" :file-scope="activeWorktree?.branch" :actions="actions" @select="pick" @select-action="pickAction" @select-file="pickFile" @close="switching = false" />
  </Transition>
</template>
