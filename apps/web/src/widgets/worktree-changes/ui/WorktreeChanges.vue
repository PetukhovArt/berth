<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from 'vue';
import { RefreshCw } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { useBerth, type PromptQuote, type SessionRef, type SessionStatus } from '../../../entities/session';
import { knownMr } from '../../../entities/merge-request';
import { GitStepButton } from '../../../features/git-step';
import { clearChangeFocus, focusedChange, quoteToPrompt } from '../../../features/session-view-switch';
import { worktreeRelative } from '../../../shared/lib/worktreePath';
import { describeError, type DiffFile, type Discussion } from '../../../shared/api/connectors/api';
import { worktreeChanges, type ChangesScope } from '../../../shared/api/fs';
import { t } from '../../../shared/lib/i18n/i18n';
import { DiffView } from '../../../features/diff-review';
import ChangesScopePicker from './ChangesScopePicker.vue';

const props = defineProps<{ path: string; status?: SessionStatus; session?: SessionRef }>();

type Changes = { kind: 'loading' } | { kind: 'failed'; message: string } | { kind: 'ready'; files: DiffFile[]; signature: string };

const NO_DISCUSSIONS: Discussion[] = [];
const NO_FILES: DiffFile[] = [];

const changes = shallowRef<Changes>({ kind: 'loading' });
const refreshing = shallowRef(false);
const scope = shallowRef<ChangesScope>('uncommitted');

let generation = 0;

const berth = useBerth();
const normal = (path: string) => path.replace(/\\/g, '/').replace(/\/+$/, '');
const worktree = computed(() => [...(berth?.entities.value.worktrees.values() ?? [])].find((entry) => normal(entry.path) === normal(props.path)));
const request = computed(() => (worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null));

function quote(entry: PromptQuote) {
  if (props.session) quoteToPrompt(props.session, entry);
}

async function load() {
  const ticket = ++generation;
  refreshing.value = true;
  try {
    const files = await worktreeChanges(props.path, scope.value);
    if (ticket !== generation) return;
    const signature = JSON.stringify(files);
    const current = changes.value;
    if (current.kind !== 'ready' || current.signature !== signature) changes.value = { kind: 'ready', files, signature };
  } catch (e) {
    if (ticket === generation) changes.value = { kind: 'failed', message: describeError(e) };
  } finally {
    if (ticket === generation) refreshing.value = false;
  }
}

watch(
  () => props.path,
  () => {
    scope.value = 'uncommitted';
    changes.value = { kind: 'loading' };
    void load();
  },
  { immediate: true },
);

watch(scope, () => {
  changes.value = { kind: 'loading' };
  void load();
});

const state = computed(() => {
  const current = changes.value;
  if (current.kind === 'loading') return { text: t('agents.changes.loading'), testid: 'worktree-changes-loading' };
  if (current.kind === 'failed') return { text: t('agents.changes.failed', { error: current.message }), testid: 'worktree-changes-error', alert: true };
  if (!current.files.length) return { text: scope.value === 'uncommitted' ? t('agents.changes.emptyUncommitted') : t('agents.changes.empty'), testid: 'worktree-changes-empty' };
  return null;
});

const diff = useTemplateRef<{ focusFile: (path: string) => boolean }>('diff');

watch(
  [() => focusedChange(props.path), changes],
  async ([file, current]) => {
    if (!file || current.kind !== 'ready') return;
    await nextTick();
    const wanted = worktreeRelative(file, props.path);
    const match = current.files.find((entry) => entry.new_path === wanted || entry.old_path === wanted) ?? current.files.find((entry) => wanted.endsWith(`/${entry.new_path}`) || entry.new_path.endsWith(`/${wanted}`));
    if (match) diff.value?.focusFile(match.new_path);
    clearChangeFocus(props.path);
  },
  { immediate: true },
);

watch(
  () => props.status,
  (status) => {
    if (status === 'finished' || status === 'needs_feedback') void load();
  },
);
</script>

<template>
  <div class="changes" data-testid="worktree-changes">
    <DiffView ref="diff" :files="changes.kind === 'ready' ? changes.files : NO_FILES" :discussions="NO_DISCUSSIONS" :root="path" :quotable="!!session" :state="state" @quote="quote">
      <template #scope>
        <ChangesScopePicker v-model="scope" :path="path" />
      </template>
      <template #state-actions>
        <VButton v-if="changes.kind === 'failed'" size="small" severity="secondary" :label="t('agents.changes.retry')" :loading="refreshing" data-testid="worktree-changes-retry" @click="load" />
        <VButton v-if="changes.kind === 'failed' || (changes.kind === 'ready' && !changes.files.length)" size="small" severity="secondary" :label="t('agents.changes.refresh')" :loading="refreshing" data-testid="worktree-changes-refresh" @click="load" />
      </template>
      <template #actions>
        <VButton v-tooltip="t('agents.changes.refresh')" size="small" severity="secondary" text :loading="refreshing" :aria-label="t('agents.changes.refresh')" data-testid="worktree-changes-refresh" @click="load">
          <template #icon><RefreshCw :size="14" /></template>
        </VButton>
        <GitStepButton v-if="worktree?.branch" quiet testid="changes-git-step" :path="worktree.path" :branch="worktree.branch" :has-request="!!request" :refresh-key="status" />
      </template>
    </DiffView>
  </div>
</template>

<style scoped>
.changes {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
</style>
