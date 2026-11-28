<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue';
import { VButton, VCheckbox, VMessage, VPopover, vTooltip } from '../../../shared/ui/kit';
import { Radio } from '../../../shared/ui/icons';
import { copyText } from '../../../shared/lib/atlassian';
import { t } from '../../../shared/lib/i18n/i18n';
import { toggleAt, type MenuAction } from '../../../shared/lib/menuAction';
import { describeError } from '../../../shared/api/connectors/api';
import { shareLink, shareState, stopSharing } from '../model/share';

const COPIED_MS = 1500;

const props = defineProps<{ agentId: string; name: string }>();

const open = ref(false);
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const shared = ref(false);
const diffs = ref(false);
const busy = ref(false);
const copied = ref(false);
const error = ref<string | null>(null);
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const label = computed(() => (shared.value ? t('remote.share.openShared') : t('remote.share.open')));

async function refresh() {
  try {
    const state = await shareState(props.agentId);
    shared.value = state.shared;
    diffs.value = state.diffs;
  } catch {
    shared.value = false;
    diffs.value = false;
  }
}

function toggle(event: Event) {
  popover.value?.toggle(event);
}

function opened() {
  open.value = true;
  error.value = null;
  void refresh();
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
    await copyText(await shareLink(props.agentId, props.name, diffs.value));
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
    diffs.value = false;
  });
}

async function changeDiffs() {
  if (!shared.value) return;
  await run(async () => {
    try {
      await shareLink(props.agentId, props.name, diffs.value);
    } catch (failure) {
      diffs.value = !diffs.value;
      throw failure;
    }
  });
}

const item = computed<MenuAction>(() => ({ label: label.value, testid: 'session-menu-share', run: (target) => toggleAt(popover.value, target) }));
defineExpose({ item });

watch(
  () => props.agentId,
  () => {
    popover.value?.hide();
    void refresh();
  },
  { immediate: true },
);
</script>

<template>
  <div class="share">
    <VButton
      v-tooltip="label"
      :class="{ filled: shared }"
      text
      severity="secondary"
      size="small"
      :aria-label="label"
      :aria-expanded="open"
      aria-haspopup="dialog"
      data-testid="share-session-button"
      @click="toggle"
    >
      <template #icon><Radio :size="13" /></template>
    </VButton>
    <VPopover ref="popover" @show="opened" @hide="open = false">
      <div class="form" data-testid="share-session-form">
        <span class="heading">{{ t('remote.share.title') }}</span>
        <span class="hint">{{ t('remote.share.hint') }}</span>
        <VCheckbox v-model="diffs" :disabled="busy" data-testid="share-session-diffs" @change="changeDiffs">
          <span class="option-text">
            <span>{{ t('remote.share.diffs') }}</span>
            <span class="hint">{{ t('remote.share.diffsHint') }}</span>
          </span>
        </VCheckbox>
        <VMessage v-if="error" severity="error" size="small" data-testid="share-session-error">{{ error }}</VMessage>
        <div class="actions">
          <VButton v-if="shared" :label="t('remote.share.stop')" severity="secondary" size="small" :disabled="busy" data-testid="share-session-stop" @click="stop" />
          <VButton :label="copied ? t('remote.share.copied') : shared ? t('remote.share.copy') : t('remote.share.start')" size="small" :disabled="busy" data-testid="share-session-copy" @click="copy" />
        </div>
      </div>
    </VPopover>
  </div>
</template>

<style scoped>
.share {
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

.option-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: var(--vr-font-size-label);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
