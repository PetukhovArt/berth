<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { Radio } from '../../../shared/ui/icons';
import { useSheetClamp } from '../../../shared/lib/useSheetClamp';
import { copyText } from '../../../shared/lib/atlassian';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError } from '../../../shared/api/connectors/api';
import { isShared, shareLink, stopSharing } from '../model/share';

const COPIED_MS = 1500;

const props = defineProps<{ agentId: string; name: string }>();

const open = ref(false);
const sheet = ref<HTMLElement | null>(null);
useSheetClamp(sheet, open);
const shared = ref(false);
const busy = ref(false);
const copied = ref(false);
const error = ref<string | null>(null);
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const label = computed(() => (shared.value ? t('remote.share.openShared') : t('remote.share.open')));

async function refresh() {
  try {
    shared.value = await isShared(props.agentId);
  } catch {
    shared.value = false;
  }
}

function toggle() {
  open.value = !open.value;
  if (open.value) {
    error.value = null;
    void refresh();
  }
}

async function run(work: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await work();
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}

function copy() {
  return run(async () => {
    await copyText(await shareLink(props.agentId, props.name));
    shared.value = true;
    copied.value = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied.value = false), COPIED_MS);
  });
}

function stop() {
  return run(async () => {
    await stopSharing(props.agentId);
    shared.value = false;
  });
}

watch(
  () => props.agentId,
  () => {
    open.value = false;
    void refresh();
  },
  { immediate: true },
);
</script>

<template>
  <div class="share">
    <UiButton type="button" class="chip" :class="{ filled: shared }" :title="label" :aria-label="label" :aria-expanded="open" data-testid="share-session-button" @click="toggle">
      <Radio :size="13" />
    </UiButton>
    <div v-if="open" ref="sheet" class="form vr-sheet" data-testid="share-session-form">
      <span class="heading">{{ t('remote.share.title') }}</span>
      <span class="hint">{{ t('remote.share.hint') }}</span>
      <span v-if="error" class="error" role="alert" data-testid="share-session-error">{{ error }}</span>
      <div class="actions">
        <UiButton v-if="shared" type="button" class="secondary" :disabled="busy" data-testid="share-session-stop" @click="stop">{{ t('remote.share.stop') }}</UiButton>
        <UiButton type="button" class="primary" :disabled="busy" data-testid="share-session-copy" @click="copy">{{ copied ? t('remote.share.copied') : shared ? t('remote.share.copy') : t('remote.share.start') }}</UiButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.share {
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
  font-size: var(--vr-font-size-label);
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
</style>
