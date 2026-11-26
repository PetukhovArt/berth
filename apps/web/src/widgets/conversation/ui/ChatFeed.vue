<script setup lang="ts">
import { Bot } from '../../../shared/ui/icons';
import { vTooltip } from '../../../shared/ui/kit';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon, modelLabel, type Checkpoint, type Harness, type Subagent } from '../../../entities/session';
import type { ChatBlock } from '../model/chatBlocks';
import { durationLabel } from '../model/duration';
import ToolGroup from './ToolGroup.vue';
import TurnFiles from './TurnFiles.vue';
import CheckpointRow from './CheckpointRow.vue';
import SubagentGroup from './SubagentGroup.vue';

withDefaults(
  defineProps<{
    blocks: readonly ChatBlock[];
    harness: Harness | null;
    agentLabel: string;
    root: string | null;
    panelId?: string;
    agentId?: string | null;
    checkpointOf?: ReadonlyMap<string, Checkpoint>;
    spoken?: ReadonlySet<string>;
    subagentOf: ReadonlyMap<string, Subagent>;
    now: number;
    subagentsOpenable?: boolean;
    found?: string | null;
  }>(),
  { panelId: undefined, agentId: null, checkpointOf: () => new Map(), spoken: () => new Set(), subagentsOpenable: true, found: null },
);
const emit = defineEmits<{ restored: []; openSubagent: [callId: string] }>();

const { t, formatDate } = useI18n();
</script>

<template>
  <ol class="messages" data-testid="conversation-messages">
    <li v-for="block in blocks" :key="block.key" class="block" :data-block="block.type" :data-key="block.key" :data-found="block.key === found || undefined">
      <div v-if="block.type === 'prompt'" v-tooltip="block.message.at ? formatDate(block.message.at) : undefined" class="message" data-role="user" data-testid="conversation-message">
        <RichContent v-if="block.message.shell && block.message.markdown" class="shell" :data-shell="block.message.shell" :markdown="block.message.markdown" />
        <p v-else class="user">{{ block.message.text }}</p>
        <span v-if="spoken.has(block.message.text.trim())" class="spoken" data-testid="conversation-spoken">{{ t('chat.sentByVoice') }}</span>
        <CheckpointRow v-if="agentId && checkpointOf.get(block.message.key)" :agent-id="agentId" :checkpoint="checkpointOf.get(block.message.key)!" @restored="emit('restored')" />
      </div>
      <div v-else-if="block.type === 'turn'" v-tooltip="block.model ? modelLabel(block.model) : undefined" class="turn" data-testid="conversation-turn">
        <HarnessIcon v-if="harness" :harness="harness" :size="14" />
        <Bot v-else :size="14" />
        <span>{{ agentLabel }}</span>
        <span v-if="block.seconds !== null" class="turn-time">· {{ durationLabel(block.seconds) }}</span>
      </div>
      <ToolGroup v-else-if="block.type === 'tools'" :calls="block.calls" :seconds="block.seconds" :root="root" />
      <SubagentGroup v-else-if="block.type === 'subagents'" :agents="block.callIds.flatMap((callId) => subagentOf.get(callId) ?? [])" :now="now" :openable="subagentsOpenable" @open="emit('openSubagent', $event)" />
      <TurnFiles v-else-if="block.type === 'files'" :files="block.files" :root="root" :panel-id="panelId" />
      <div v-else v-tooltip="block.message.at ? formatDate(block.message.at) : undefined" class="message" :data-role="block.message.role" data-testid="conversation-message">
        <RichContent v-if="block.message.shell && block.message.markdown" class="shell" :data-shell="block.message.shell" :markdown="block.message.markdown" />
        <p v-else-if="block.message.role === 'user'" class="user">{{ block.message.text }}</p>
        <RichContent v-else class="assistant" :markdown="block.message.text" />
      </div>
    </li>
  </ol>
</template>

<style scoped>
.messages {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin-top: 0;
  margin-bottom: 0;
  padding: 0;
  list-style: none;
}

.messages :deep(.rich-content .body :is(p, ul, ol, dl, blockquote, h1, h2, h3, h4, h5, h6)) {
  max-width: none;
}

.block[data-block='prompt'] {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  align-self: flex-end;
  max-width: 85%;
}

.block[data-block='turn'] {
  margin-top: var(--vr-space-2);
}

.message[data-role='user'] {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.block > .message {
  border-radius: var(--vr-radius-lg);
  transition:
    background-color var(--vr-motion-slow) var(--vr-motion-ease-standard),
    box-shadow var(--vr-motion-slow) var(--vr-motion-ease-standard);
}

.block[data-found] > .message {
  background-color: var(--vr-color-match-bg);
  box-shadow: 0 0 0 var(--vr-space-1) var(--vr-color-match-bg);
}

.spoken {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-subtle);
}

.turn {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.turn-time {
  color: var(--vr-color-text-subtle);
}

.user {
  margin: 0;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-lg);
}

.shell {
  font-size: var(--vr-font-size-xs);
}

.shell[data-shell='output'] {
  color: var(--vr-color-text-muted);
}

.assistant {
  font-size: var(--vr-font-size-sm);
}
</style>
