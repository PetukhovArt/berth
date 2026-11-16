<script setup lang="ts">
import { computed } from 'vue';
import { Mic } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { pushHolding, pushSession, stopWakeWord, voiceNotice, voiceTranscript, wakeArmed, wakePhrase, wakeSession } from '../model/voiceInput';

const status = computed(() => {
  if (pushHolding.value) return t('voice.holding', { name: pushSession.value?.name ?? '' });
  const session = wakeSession.value;
  if (!session) return '';
  return wakeArmed.value ? t('voice.armed', { name: session.name }) : t('voice.wake', { phrase: wakePhrase.value, name: session.name });
});
</script>

<template>
  <div v-if="status || voiceNotice" class="voice" role="status" data-testid="voice-indicator" :data-mode="pushHolding ? 'push' : wakeSession ? 'wake' : undefined">
    <template v-if="status">
      <Mic class="mic" :size="14" />
      <span class="status" data-testid="voice-status">{{ status }}</span>
      <span v-if="voiceTranscript" class="transcript" data-testid="voice-transcript">{{ voiceTranscript }}</span>
      <UiButton v-if="wakeSession && !pushHolding" type="button" class="off" data-testid="voice-wake-off" @click="stopWakeWord">{{ t('voice.off') }}</UiButton>
    </template>
    <span v-else class="notice" data-testid="voice-notice">{{ voiceNotice }}</span>
  </div>
</template>

<style scoped>
.voice {
  position: fixed;
  bottom: var(--vr-space-6);
  left: 50%;
  z-index: 1100;
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  max-width: min(640px, calc(100vw - 2 * var(--vr-space-4)));
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
  transform: translateX(-50%);
}

.mic {
  flex: none;
  color: var(--vr-color-danger);
}

.status {
  flex: none;
}

.transcript {
  overflow: hidden;
  color: var(--vr-color-text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.off {
  flex: none;
  padding: 0;
  font: inherit;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
}

.off:hover {
  color: var(--vr-color-text);
}
</style>
