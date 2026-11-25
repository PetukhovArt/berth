<script setup lang="ts">
import { shallowRef, useTemplateRef } from 'vue';
import { Plug, RefreshCw } from '../../../shared/ui/icons';
import { VPopover } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import type { AcpMcpServer } from '../../../entities/session';

const props = defineProps<{ servers: readonly AcpMcpServer[]; refresh: () => Promise<void> }>();

const { t } = useI18n();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const refreshing = shallowRef(false);

async function reload() {
  refreshing.value = true;
  try {
    await props.refresh();
  } finally {
    refreshing.value = false;
  }
}

function label(status: string | null): string {
  switch (status) {
    case null:
      return t('chat.mcp.configured');
    case 'connected':
      return t('chat.mcp.connected');
    case 'failed':
      return t('chat.mcp.failed');
    case 'pending':
      return t('chat.mcp.pending');
    case 'needs-auth':
      return t('chat.mcp.needsAuth');
    case 'disabled':
      return t('chat.mcp.disabled');
    default:
      return status;
  }
}

function tone(status: string | null): string {
  if (status === 'connected') return 'ok';
  if (status === 'failed' || status === 'needs-auth') return 'bad';
  if (status === 'pending') return 'wait';
  return 'idle';
}
</script>

<template>
  <UiButton type="button" class="picker" :title="t('chat.mcp.title')" aria-haspopup="dialog" data-testid="chat-mcp" @click="popover?.toggle($event)">
    <Plug :size="12" />
    <span class="count">{{ servers.length }}</span>
  </UiButton>
  <VPopover ref="popover">
    <div class="list" role="dialog" :aria-label="t('chat.mcp.title')" data-testid="chat-mcp-list">
      <div v-for="server in servers" :key="server.name" class="item" data-testid="chat-mcp-server" :data-status="server.status ?? 'configured'">
        <span class="dot" :data-tone="tone(server.status)" />
        <span class="name">{{ server.name }}</span>
        <span class="state">{{ label(server.status) }}</span>
      </div>
      <UiButton type="button" class="refresh" :disabled="refreshing" :aria-busy="refreshing" data-testid="chat-mcp-refresh" @click="reload">
        <RefreshCw :size="13" />
        {{ t('chat.mcp.refresh') }}
      </UiButton>
    </div>
  </VPopover>
</template>

<style scoped>
.picker {
  display: inline-flex;
  flex: none;
  gap: var(--vr-space-1);
  align-items: center;
  height: 26px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-control);
}

.picker:hover {
  color: var(--vr-color-text);
}

.list {
  display: flex;
  flex-direction: column;
  min-width: 220px;
  max-height: min(320px, 45vh);
  overflow-y: auto;
}

.item,
.refresh {
  display: flex;
  gap: var(--vr-space-2);
  align-items: center;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text);
}

.refresh {
  text-align: left;
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.refresh:hover:not(:disabled),
.refresh:focus-visible {
  background: var(--vr-color-nav-hover);
}

.refresh:disabled {
  color: var(--vr-color-text-subtle);
  cursor: default;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  background: var(--vr-color-text-subtle);
  border-radius: 50%;
}

.dot[data-tone='ok'] {
  background: var(--vr-status-finished);
}

.dot[data-tone='bad'] {
  background: var(--vr-status-crashed);
}

.dot[data-tone='wait'] {
  background: var(--vr-status-running);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.state {
  flex: none;
  color: var(--vr-color-text-muted);
}
</style>
