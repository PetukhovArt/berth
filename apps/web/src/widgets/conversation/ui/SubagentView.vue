<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, shallowRef, useTemplateRef, watch } from 'vue';
import { ChevronLeft } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { describeError, deriveSubagents, modelLabel, readSubagentTranscript, subagentSeconds, type Harness, type Subagent, type TranscriptMessage } from '../../../entities/session';
import { chatBlocks } from '../model/chatBlocks';
import { durationLabel } from '../model/duration';
import ChatFeed from './ChatFeed.vue';

const TAIL = 400;
const PIN_PX = 48;
const POLL_MS = 2000;

const props = defineProps<{ harness: Harness; sessionId: string; agent: Subagent; agentLabel: string; root: string | null; panelId?: string; now: number }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const scroller = useTemplateRef<HTMLElement>('scroller');
const messages = shallowRef<TranscriptMessage[]>([]);
const loading = shallowRef(true);
const error = shallowRef<string | null>(null);
const missing = shallowRef(false);
const running = computed(() => props.agent.state === 'running');
const blocks = computed(() => chatBlocks(messages.value));
const nested = computed(() => new Map(deriveSubagents(messages.value, running.value).all.map((entry) => [entry.callId, entry])));
const seconds = computed(() => subagentSeconds(props.agent, props.now));
let generation = 0;
let timer: ReturnType<typeof setInterval> | undefined;

async function load() {
  const current = ++generation;
  const el = scroller.value;
  const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < PIN_PX;
  try {
    const next = await readSubagentTranscript({ harness: props.harness, id: props.sessionId }, props.agent.callId, TAIL);
    if (current !== generation) return;
    missing.value = next === null;
    messages.value = next ?? [];
    error.value = null;
  } catch (e) {
    if (current === generation) error.value = describeError(e);
  } finally {
    if (current === generation) loading.value = false;
  }
  if (!pinned || current !== generation) return;
  await nextTick();
  const box = scroller.value;
  if (box) box.scrollTop = box.scrollHeight;
}

watch(
  () => [props.sessionId, props.agent.callId] as const,
  () => {
    loading.value = true;
    void load();
  },
  { immediate: true },
);

watch(
  running,
  (on, was) => {
    clearInterval(timer);
    timer = on ? setInterval(() => void load(), POLL_MS) : undefined;
    if (was && !on) void load();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  generation += 1;
  clearInterval(timer);
});
</script>

<template>
  <section class="view" data-testid="subagent-view" :data-call-id="agent.callId" :data-state="agent.state">
    <header class="head">
      <VButton class="back" :label="t('chat.subagents.back')" text severity="secondary" size="small" data-testid="subagent-view-back" @click="emit('close')">
        <template #icon><ChevronLeft :size="16" /></template>
      </VButton>
      <div class="title">
        <span v-tooltip="agent.description" class="name" data-testid="subagent-view-title">{{ agent.description }}</span>
        <span class="meta">
          <span>{{ agent.kind ?? t('chat.subagents.noType') }}</span>
          <span v-if="agent.model">{{ modelLabel(agent.model) }}</span>
          <span v-if="seconds !== null" class="time">{{ durationLabel(seconds) }}</span>
          <span data-testid="subagent-view-state">{{ t(`chat.subagents.state.${agent.state}`) }}</span>
        </span>
      </div>
    </header>
    <div ref="scroller" class="scroll" data-testid="subagent-view-scroll">
      <p v-if="loading" class="state">{{ t('workbench.external.loading') }}</p>
      <p v-else-if="error" class="state error" role="alert">{{ error }}</p>
      <p v-else-if="missing" class="state" :data-testid="running ? 'subagent-view-pending' : 'subagent-view-missing'">{{ running ? t('chat.subagents.transcriptPending') : t('chat.subagents.transcriptMissing') }}</p>
      <p v-else-if="!messages.length" class="state">{{ t('workbench.external.empty') }}</p>
      <ChatFeed v-else class="feed" :blocks="blocks" :harness="harness" :agent-label="agentLabel" :root="root" :panel-id="panelId" :subagent-of="nested" :now="now" :subagents-openable="false" />
    </div>
  </section>
</template>

<style scoped>
.view {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  background: var(--vr-color-surface);
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.back {
  flex: none;
  min-height: var(--vr-size-row);
}

:root[data-vr-viewport='phone'] .back {
  min-height: var(--vr-size-touch);
}

.title {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  overflow: hidden;
  font-size: var(--vr-font-size-label);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.time {
  font-variant-numeric: tabular-nums;
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-6);
}

.state {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.state.error {
  color: var(--vr-color-danger);
}
</style>
