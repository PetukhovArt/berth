<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import { AGENT_KIND_LABEL, getMetrics, isFiveHourWindow, isMonthWindow, sessionKey, useUsageLimits, type AgentKind, type UsageWindow } from '../../../entities/session';
import InspectorSection from './InspectorSection.vue';

const REFRESH_MS = 15_000;
const WEEK_MINUTES = 10080;

const props = defineProps<{ panelKey: string; kind: AgentKind }>();

const { t, formatDate, formatNumber } = useI18n();
const usage = useUsageLimits();
const scope = useScope();
const tokens = shallowRef<number | null>(null);

function windowLabel(window: UsageWindow): string {
  if (isFiveHourWindow(window)) return t('agents.metrics.windowFive');
  if (window.window_minutes === WEEK_MINUTES) return t('agents.metrics.windowWeek');
  if (isMonthWindow(window)) return t('agents.metrics.windowMonth');
  return t('agents.metrics.windowOther', { hours: Math.round(window.window_minutes / 60) });
}

const windows = computed(() =>
  usage.windows.value
    .filter((window) => window.kind === props.kind)
    .map((window) => {
      const percent = Math.min(100, Math.max(0, Math.round(window.used_percent)));
      const time = formatDate(window.resets_at_ms, { weekday: isFiveHourWindow(window) ? undefined : 'short', hour: '2-digit', minute: '2-digit' });
      return { key: `${window.kind}:${window.window_minutes}`, label: windowLabel(window), percent, meta: t('inspector.usage.resets', { percent, time }) };
    }),
);

async function refreshTokens() {
  const key = props.panelKey;
  try {
    const snapshot = await scope.load(getMetrics());
    if (key !== props.panelKey) return;
    const own = snapshot.sessions.find((metrics) => sessionKey(metrics.session) === key);
    tokens.value = own?.tokens ? own.tokens.by_model.reduce((sum, model) => sum + model.input + model.output + model.cache_read + model.cache_write_5m + model.cache_write_1h, 0) : null;
  } catch {
    if (key === props.panelKey) tokens.value = null;
  }
}

watch(
  () => props.panelKey,
  () => {
    tokens.value = null;
    void refreshTokens();
  },
  { immediate: true },
);
visibleInterval(scope, REFRESH_MS, () => void refreshTokens());
</script>

<template>
  <InspectorSection :title="t('inspector.usage.title')" :meta="AGENT_KIND_LABEL[kind]" testid="inspector-usage">
    <div v-if="tokens !== null" class="row" data-testid="inspector-usage-tokens">
      <span class="grow">{{ t('inspector.usage.session') }}</span>
      <span class="end">{{ t('inspector.usage.tokens', { tokens: formatNumber(tokens, { notation: 'compact' }) }) }}</span>
    </div>
    <div v-for="window in windows" :key="window.key" class="window" data-testid="inspector-usage-window">
      <div class="row">
        <span class="grow">{{ window.label }}</span>
        <span class="end">{{ window.meta }}</span>
      </div>
      <div class="bar" role="progressbar" :aria-label="window.label" :aria-valuenow="window.percent" aria-valuemin="0" aria-valuemax="100">
        <div class="fill" :data-spent="window.percent >= 100" :style="{ width: `${window.percent}%` }" />
      </div>
    </div>
    <p v-if="tokens === null && !windows.length" class="note">{{ t('inspector.usage.none') }}</p>
  </InspectorSection>
</template>

<style scoped>
.window {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bar {
  height: 4px;
  overflow: hidden;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-surface-muted);
}

.fill {
  height: 100%;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-text-muted);
}

.fill[data-spent='true'] {
  background: var(--vr-status-limit);
}
</style>
