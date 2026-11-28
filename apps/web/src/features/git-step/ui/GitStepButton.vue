<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { VButton, VInputText, VMessage, VTextarea, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { forgeCreateChangeRequest, worktreeGitState, worktreeGitStep, type GitState } from '../../../shared/api/git';
import { rememberMr } from '../../../entities/merge-request';
import { GitBranch } from '../../../shared/ui/icons';
import { useSheet } from '../../../shared/lib/useSheet';
import { createRequested } from '../model/createRequest';
import { idleHintKey, nextStep, stepLabelKey, stepShortLabelKey } from '../model/nextStep';

const props = withDefaults(defineProps<{ path: string; branch: string; hasRequest: boolean; refreshKey?: string; testid?: string; quiet?: boolean }>(), { testid: 'git-step', refreshKey: undefined });

const state = ref<GitState | null>(null);
const busy = ref(false);
const error = ref<string | null>(null);
const open = ref(false);
const sheet = ref<HTMLElement | null>(null);
useSheet(sheet, open);
const message = ref('');
const title = ref('');
const description = ref('');

const step = computed(() => (state.value ? nextStep(state.value, props.hasRequest) : null));
const runnable = computed(() => step.value && step.value.kind !== 'idle');
const label = computed(() => (step.value ? t(stepLabelKey(step.value)) : ''));
const shortLabel = computed(() => (step.value ? t(stepShortLabelKey(step.value)) : ''));
const needsForm = computed(() => step.value?.kind === 'run' && (step.value.commit || step.value.create));
const hint = computed(() => {
  const current = step.value;
  if (!current || !state.value) return '';
  if (current.kind === 'idle') return t(idleHintKey(current.reason));
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
watch(createRequested, async (request) => {
  if (!request || request.path !== props.path) return;
  await refresh();
  const current = step.value;
  if (current?.kind === 'run' && current.create && !busy.value) open.value = true;
});

onMounted(() => window.addEventListener('focus', refresh));
onBeforeUnmount(() => window.removeEventListener('focus', refresh));
</script>

<template>
  <div v-if="step" class="git-step" :data-testid="testid">
    <span v-if="hint && !quiet" v-tooltip="hint" class="hint" :data-testid="`${testid}-hint`">{{ hint }}</span>
    <VButton v-tooltip="[label, hint].filter(Boolean).join('\n')" type="button" size="small" class="run" :disabled="!runnable || busy" :aria-busy="busy" :aria-label="busy ? t('gitStep.working') : label" :data-testid="`${testid}-run`" @click="click">
      <GitBranch class="glyph" :size="14" aria-hidden="true" />
      <span class="full">{{ busy ? t('gitStep.working') : label }}</span>
      <span class="short">{{ busy ? t('gitStep.working') : shortLabel }}</span>
    </VButton>
    <form v-if="open && step.kind === 'run'" ref="sheet" class="form vr-sheet" :data-testid="`${testid}-form`" @submit.prevent="valid && perform()">
      <label v-if="step.commit" class="field">
        <span>{{ t('gitStep.commitMessage') }}</span>
        <VInputText v-model="message" :data-testid="`${testid}-message`" autofocus />
      </label>
      <template v-if="step.create">
        <label class="field">
          <span>{{ t('gitStep.requestTitle') }}</span>
          <VInputText v-model="title" :data-testid="`${testid}-title`" />
        </label>
        <label class="field">
          <span>{{ t('gitStep.requestDescription') }}</span>
          <VTextarea v-model="description" rows="3" :data-testid="`${testid}-description`" />
        </label>
      </template>
      <div class="actions">
        <VButton type="button" size="small" severity="secondary" text :label="t('gitStep.cancel')" @click="open = false" />
        <VButton type="submit" size="small" :label="label" :disabled="!valid || busy" :data-testid="`${testid}-submit`" />
      </div>
    </form>
    <VMessage v-if="error" class="error" severity="error" size="small" :data-testid="`${testid}-error`">{{ error }}</VMessage>
  </div>
</template>

<style scoped>
.git-step {
  position: relative;
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.hint,
.error {
  font-size: var(--vr-font-size-xs);
}

.hint {
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  color: var(--vr-color-text-subtle);
  text-overflow: ellipsis;
}

.error {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 6;
  width: max-content;
  max-width: 360px;
  max-height: 9em;
  overflow-y: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.run {
  flex: none;
  white-space: nowrap;
}

.glyph,
.short {
  display: none;
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

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
