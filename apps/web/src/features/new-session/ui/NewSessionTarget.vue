<script setup lang="ts">
import { computed } from 'vue';
import { VMessage, VSelect, VTextarea } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth } from '../../../entities/session';
import type { BranchMode, SessionTarget } from '../model/sessionTarget';

const props = defineProps<{ state: SessionTarget; projectLocked: boolean }>();
const emit = defineEmits<{ enter: [] }>();

function onBranchEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  event.preventDefault();
  emit('enter');
}
const berth = useBerth();
const { project, target, mode, existing, fresh, setup, base, baseOptions, baseChoice, branches, branchesError, loading, count, sharesBranch, projects, targets, creating, plan } = props.state;

const baseModel = computed({
  get: () => base.value,
  set: (value: string | null) => {
    baseChoice.value = value;
  },
});

const MODES = computed<{ label: string; value: BranchMode; disabled?: boolean }[]>(() => [
  { label: t('agents.newSession.branchCurrent'), value: 'current' },
  { label: t('agents.newSession.branchExisting'), value: 'existing', disabled: !branches.value },
  { label: t('agents.newSession.branchNew'), value: 'new' },
]);

const branchOptions = computed(() =>
  (branches.value ?? []).map((branch) => {
    const holder = branch.worktree ? berth?.entities.value.worktrees.get(branch.worktree) : undefined;
    const where = holder ? (holder.is_main ? t('agents.newSession.inMain') : t('agents.newSession.inWorktree')) : t('agents.newSession.noCheckout');
    return { label: `${branch.name} · ${where}`, value: branch.name };
  }),
);

const projectLabel = computed(() => projects.value.find((each) => each.id === project.value)?.label ?? '');

const setupNote = computed(() => (plan.value.kind === 'create' && setup.value.trim() ? t('agents.newSession.planSetup', { command: setup.value.trim() }) : ''));

const summary = computed(() => {
  const current = plan.value;
  if (current.kind === 'incomplete') return current.reason;
  if (current.kind === 'use') return t(current.worktree.is_main ? 'agents.newSession.planMain' : 'agents.newSession.planUse', { branch: current.worktree.branch });
  if (!current.branch) return current.base ? t('agents.newSession.planCreateGeneratedFrom', { base: current.base }) : t('agents.newSession.planCreateGenerated');
  if (count.value > 1 && !sharesBranch.value) return t('agents.newSession.planCreateEach');
  if (current.fetch && current.base) return t('agents.newSession.planCreateFresh', { base: current.base });
  return current.base ? t('agents.newSession.planCreateFrom', { base: current.base }) : t('agents.newSession.planCreate');
});
</script>

<template>
  <div class="target" data-testid="new-session-target">
    <div class="chips">
      <VSelect v-if="!projectLocked" v-model="project" :options="projects" option-label="label" option-value="id" size="small" class="chip" :aria-label="t('agents.newSession.project')" data-testid="new-session-project" />
      <span v-else class="chip fixed" data-testid="new-session-project-name">{{ projectLabel }}</span>
      <VSelect v-model="target" :options="targets" option-label="label" option-value="id" size="small" class="chip" :aria-label="t('agents.newSession.target')" data-testid="new-session-worktree" />
      <VSelect
        v-if="creating && baseOptions.length"
        v-model="baseModel"
        :options="baseOptions"
        option-label="label"
        option-value="id"
        filter
        size="small"
        class="chip"
        :aria-label="t('agents.newSession.base')"
        data-testid="new-session-base"
      >
        <template #value="{ value }">{{ t('agents.newSession.branchFrom', { base: value }) }}</template>
      </VSelect>
      <VSelect v-if="!creating" v-model="mode" :options="MODES" option-label="label" option-value="value" option-disabled="disabled" size="small" class="chip" :aria-label="t('agents.newSession.branch')" data-testid="new-session-branch-mode" />
      <slot name="type" />
    </div>
    <label v-if="!creating && mode === 'existing'" class="row">
      <span class="row-label">{{ t('agents.newSession.existingBranch') }}</span>
      <VSelect
        v-model="existing"
        :options="branchOptions"
        option-label="label"
        option-value="value"
        filter
        size="small"
        :loading="loading"
        :placeholder="t('agents.newSession.pickBranch')"
        :aria-label="t('agents.newSession.existingBranch')"
        class="row-input"
        data-testid="new-session-existing-branch"
      />
    </label>
    <label v-if="creating || mode === 'new'" class="row">
      <span class="row-label">{{ t('agents.newSession.newBranch') }}</span>
      <VTextarea v-model="fresh" rows="1" auto-resize size="small" :placeholder="t('agents.newSession.typeBranch')" class="row-input mono branch" data-testid="new-session-new-branch" @keydown.enter.exact="onBranchEnter" />
    </label>
    <VMessage v-if="branchesError" severity="warn" size="small" data-testid="new-session-branches-error">{{ t('agents.newSession.branchesFailed', { error: branchesError }) }}</VMessage>
    <div class="summary" :data-plan="plan.kind" data-testid="new-session-plan">
      <svg class="arrow" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" /></svg>
      <div>
        <span>{{ summary }}</span>
        <div v-if="setupNote" data-testid="new-session-setup-plan" class="setup">{{ setupNote }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.target {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  height: 28px;
  min-width: 0;
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
  box-shadow: none;
  font-size: var(--vr-font-size-label);
}

.fixed {
  display: inline-flex;
  align-items: center;
  padding: 0 9px;
  font-size: var(--vr-font-size-sm);
  line-height: 1;
  color: var(--vr-color-text);
  white-space: nowrap;
}

.chip :deep(.p-select-label) {
  display: flex;
  align-items: center;
  align-self: stretch;
  padding: 0 9px;
  white-space: nowrap;
}

.chip :deep(.p-select-dropdown) {
  width: 1.6rem;
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
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.row-input {
  flex: 1;
  min-width: 0;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-label);
}

.branch {
  resize: none;
  overflow-wrap: anywhere;
}

.summary {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: var(--vr-font-size-label);
  line-height: 1.4;
  color: var(--vr-color-text-muted);
}

.setup {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.arrow {
  flex: none;
  margin-top: 2px;
  color: currentColor;
}

:root[data-vr-viewport='phone'] .chips {
  flex-direction: column;
}

:root[data-vr-viewport='phone'] .chip {
  height: auto;
  min-height: var(--vr-size-row-touch);
}

:root[data-vr-viewport='phone'] .chip :deep(.p-select-label) {
  min-height: var(--vr-size-row-touch);
  white-space: normal;
  overflow-wrap: anywhere;
}

:root[data-vr-viewport='phone'] .fixed {
  min-height: var(--vr-size-row-touch);
  white-space: normal;
  overflow-wrap: anywhere;
}

:root[data-vr-viewport='phone'] .row {
  flex-direction: column;
  align-items: stretch;
  gap: var(--vr-space-1);
}

:root[data-vr-viewport='phone'] .row-label {
  width: auto;
}
</style>
