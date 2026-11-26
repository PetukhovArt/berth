<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { Unplug } from '../../../shared/ui/icons';
import { VButton, VPopover } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { openSettings } from '../../../shared/model/nav';
import { useWorkTree } from '../../../features/work-tree';

const { t, formatTime } = useI18n();
const { failures, retryConnector } = useWorkTree();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const retrying = shallowRef<ReadonlySet<string>>(new Set());

const count = computed(() => failures.value.length);
const label = computed(() => t('titleBar.connectors.unavailable', { count: count.value }));
const tooltip = computed(() => `${label.value}. ${t('titleBar.connectors.tooltip', { names: failures.value.map((failure) => failure.name).join(', ') })}`);
const vpnHint = computed(() => failures.value.length > 1 && failures.value.every((failure) => failure.unreachable));

async function retry(id: string) {
  retrying.value = new Set([...retrying.value, id]);
  try {
    await retryConnector(id);
  } finally {
    retrying.value = new Set([...retrying.value].filter((each) => each !== id));
  }
}

function configure() {
  popover.value?.hide();
  openSettings('connectors');
}
</script>

<template>
  <template v-if="count">
    <UiButton type="button" class="connector-errors" :title="tooltip" :aria-label="label" aria-haspopup="dialog" data-testid="titlebar-connector-errors" @click="popover?.toggle($event)">
      <Unplug :size="15" :stroke-width="1.6" />
      <span class="badge" data-testid="titlebar-connector-errors-count">{{ count }}</span>
    </UiButton>
    <VPopover ref="popover">
      <div class="panel" role="dialog" :aria-label="t('titleBar.connectors.title', { count })" data-testid="connector-errors-panel">
        <p class="title">{{ t('titleBar.connectors.title', { count }) }}</p>
        <section v-for="failure in failures" :key="failure.id" class="card" data-testid="connector-error">
          <h3 class="name"><span class="dot" />{{ failure.name }}</h3>
          <p class="message">{{ failure.message }}</p>
          <p class="since">{{ t('titleBar.connectors.since', { time: formatTime(failure.since) }) }}</p>
          <div class="actions">
            <VButton :label="t('titleBar.connectors.retry')" size="small" severity="secondary" :disabled="retrying.has(failure.id)" :loading="retrying.has(failure.id)" data-testid="connector-error-retry" @click="retry(failure.id)" />
            <VButton :label="t('titleBar.connectors.configure')" size="small" severity="secondary" text data-testid="connector-error-configure" @click="configure" />
          </div>
        </section>
        <p v-if="vpnHint" class="title" data-testid="connector-errors-vpn">{{ t('titleBar.connectors.vpnHint') }}</p>
      </div>
    </VPopover>
  </template>
</template>

<style scoped>
.connector-errors {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  height: 26px;
  min-width: 28px;
  padding: 0 6px;
  color: var(--vr-color-danger);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.connector-errors:hover,
.connector-errors:focus-visible {
  background: var(--vr-color-nav-hover);
}

.badge {
  position: absolute;
  top: -2px;
  right: -2px;
  min-width: 1.45em;
  padding: 0 0.25em;
  font-size: var(--vr-font-size-2xs);
  font-weight: 700;
  line-height: 1.45;
  text-align: center;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
  border-radius: var(--vr-radius-full);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: min(22rem, 80vw);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: var(--vr-space-2) var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-lg);
}

.name {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-label);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  background: var(--vr-color-danger);
  border-radius: var(--vr-radius-full);
}

.message {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  overflow-wrap: anywhere;
  user-select: text;
}

.since {
  margin: 0;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-subtle);
}

.actions {
  display: flex;
  gap: var(--vr-space-2);
  margin-top: var(--vr-space-1);
}
</style>
