<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { forgeCreateChangeRequest, worktreeGitState, worktreeGitStep, type GitState } from '../../../shared/api/git';
import { rememberMr } from '../../../entities/merge-request';
import { nextStep, stepLabelKey } from '../model/nextStep';

const props = defineProps<{ path: string; branch: string; hasRequest: boolean; refreshKey?: string }>();

const state = ref<GitState | null>(null);
const busy = ref(false);
const error = ref<string | null>(null);
const open = ref(false);
const message = ref('');
const title = ref('');
const description = ref('');

const step = computed(() => (state.value ? nextStep(state.value, props.hasRequest) : null));
const runnable = computed(() => step.value && step.value.kind !== 'idle');
const label = computed(() => (step.value ? t(stepLabelKey(step.value)) : ''));
const needsForm = computed(() => step.value?.kind === 'run' && (step.value.commit || step.value.create));
const hint = computed(() => {
  const current = step.value;
  if (!current || !state.value) return '';
  if (current.kind === 'idle') return t(current.reason === 'defaultBranch' ? 'gitStep.defaultBranch' : 'gitStep.upToDate');
  if (current.kind === 'pull') return t('gitStep.behind', { count: state.value.behind });
  return '';
});
const valid = computed(() => {
  const current = step.value;
  if (current?.kind !== 'run') return true;
  return (!current.commit || message.value.trim() !== '') && (!current.create || title.value.trim() !== '');
});

let seq = 0;
async function refresh() {
  const id = ++seq;
  try {
    const next = await worktreeGitState(props.path);
    if (id === seq) state.value = next;
  } catch {
    if (id === seq) state.value = null;
  }
}

async function perform() {
  const current = step.value;
  if (!current || current.kind === 'idle' || busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    if (current.kind === 'pull') {
      await worktreeGitStep(props.path, 'pull');
    } else {
      if (current.commit) await worktreeGitStep(props.path, 'commit', message.value);
      if (current.push) await worktreeGitStep(props.path, 'push');
      if (current.create && state.value?.base) {
        const created = await forgeCreateChangeRequest(props.path, props.branch, state.value.base, title.value.trim(), description.value);
        rememberMr(props.path, props.branch, created);
      }
    }
    open.value = false;
    message.value = '';
    title.value = '';
    description.value = '';
  } catch (failure) {
    error.value = String(failure);
  } finally {
    busy.value = false;
    await refresh();
  }
}

function click() {
  if (!runnable.value || busy.value) return;
  if (needsForm.value) {
    open.value = !open.value;
    return;
  }
  void perform();
}

watch(
  () => [props.path, props.refreshKey] as const,
  () => {
    open.value = false;
    error.value = null;
    void refresh();
  },
  { immediate: true },
);
watch(
  () => props.hasRequest,
  () => void refresh(),
);

onMounted(() => window.addEventListener('focus', refresh));
onBeforeUnmount(() => window.removeEventListener('focus', refresh));
</script>

<template>
  <div v-if="step" class="git-step" data-testid="git-step">
    <span v-if="hint" class="hint" data-testid="git-step-hint">{{ hint }}</span>
    <UiButton type="button" class="run" :disabled="!runnable || busy" :aria-busy="busy" data-testid="git-step-run" @click="click">
      {{ busy ? t('gitStep.working') : label }}
    </UiButton>
    <form v-if="open && step.kind === 'run'" class="form vr-sheet" data-testid="git-step-form" @submit.prevent="valid && perform()">
      <label v-if="step.commit" class="field">
        <span>{{ t('gitStep.commitMessage') }}</span>
        <UiInput v-model="message" data-testid="git-step-message" autofocus />
      </label>
      <template v-if="step.create">
        <label class="field">
          <span>{{ t('gitStep.requestTitle') }}</span>
          <UiInput v-model="title" data-testid="git-step-title" />
        </label>
        <label class="field">
          <span>{{ t('gitStep.requestDescription') }}</span>
          <UiTextarea v-model="description" rows="3" data-testid="git-step-description" />
        </label>
      </template>
      <div class="actions">
        <UiButton type="button" class="cancel" @click="open = false">{{ t('gitStep.cancel') }}</UiButton>
        <UiButton type="submit" class="run" :disabled="!valid || busy" data-testid="git-step-submit">{{ label }}</UiButton>
      </div>
    </form>
    <span v-if="error" class="error" role="alert" data-testid="git-step-error">{{ error }}</span>
  </div>
</template>

<style scoped>
.git-step {
  position: relative;
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
}

.hint,
.error {
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
}

.hint {
  color: var(--vr-color-text-subtle);
}

.error {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 6;
  max-width: 360px;
  overflow: hidden;
  color: var(--vr-status-crashed);
  text-overflow: ellipsis;
}

.run {
  height: 28px;
  padding: 0 11px;
  color: var(--vr-color-accent-contrast);
  font: inherit;
  font-size: 12.5px;
  font-weight: 600;
  white-space: nowrap;
  background: var(--vr-color-accent);
  border: 0;
  border-radius: 7px;
  cursor: pointer;
}

.run:disabled {
  color: var(--vr-color-text-subtle);
  background: var(--vr-color-surface-muted);
  cursor: default;
}

.form {
  position: absolute;
  top: 34px;
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

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.field :deep(input),
.field :deep(textarea) {
  padding: 6px 8px;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border);
  border-radius: 6px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.cancel {
  height: 28px;
  padding: 0 10px;
  color: var(--vr-color-text);
  font: inherit;
  font-size: 12.5px;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: 7px;
  cursor: pointer;
}
</style>
