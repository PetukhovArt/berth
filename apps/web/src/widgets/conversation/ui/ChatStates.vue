<script setup lang="ts">
import { TriangleAlert } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';

defineProps<{ state: 'loading' | 'empty' | 'error'; message?: string | null; canShowTerminal?: boolean; canShowAgentLog?: boolean }>();
const emit = defineEmits<{ retry: []; showTerminal: []; showAgentLog: [] }>();
</script>

<template>
  <div v-if="state === 'loading'" class="loading" aria-busy="true" data-testid="chat-loading">
    <p class="hint">{{ t('workbench.external.loading') }}</p>
    <i class="bubble wide" />
    <i class="line" />
    <i class="line short" />
    <i class="bubble" />
    <i class="line mid" />
  </div>
  <p v-else-if="state === 'empty'" class="empty" data-testid="chat-empty">{{ t('workbench.external.empty') }}</p>
  <div v-else class="error" role="alert" data-testid="chat-error">
    <TriangleAlert :size="22" class="icon" />
    <p class="title">{{ t('chat.state.errorTitle') }}</p>
    <p class="detail">{{ message }}</p>
    <div class="actions">
      <VButton :label="t('chat.state.retry')" size="small" data-testid="chat-retry" @click="emit('retry')" />
      <VButton v-if="canShowTerminal" :label="t('chat.state.showTerminal')" severity="secondary" size="small" data-testid="chat-show-terminal" @click="emit('showTerminal')" />
      <VButton v-if="canShowAgentLog" :label="t('chat.state.showAgentLog')" severity="secondary" size="small" data-testid="chat-show-agent-log" @click="emit('showAgentLog')" />
    </div>
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.hint {
  margin: 0;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
}

.bubble,
.line {
  display: block;
  background: var(--vr-color-surface-muted);
  animation: pulse 1.2s ease-in-out infinite;
}

.bubble {
  align-self: flex-end;
  width: 34%;
  height: 44px;
  border-radius: var(--vr-radius-bubble);
}

.bubble.wide {
  width: 46%;
}

.line {
  width: 80%;
  height: 12px;
  margin-top: -10px;
  border-radius: var(--vr-radius-sm);
}

.line.short {
  width: 64%;
}

.line.mid {
  width: 72%;
}

.empty {
  margin: var(--vr-space-8) 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
  text-align: center;
}

.error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-2);
  margin: var(--vr-space-8) auto;
  text-align: center;
}

.icon {
  color: var(--vr-status-crashed);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-title);
  font-weight: 600;
}

.detail {
  max-width: 420px;
  margin: 0;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  gap: var(--vr-space-2);
  margin-top: var(--vr-space-2);
}

@keyframes pulse {
  50% {
    opacity: 0.4;
  }
}
</style>
