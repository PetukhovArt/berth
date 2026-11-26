<script setup lang="ts">
import { computed } from 'vue';
import { Bot, Check, LoaderCircle } from '../../../shared/ui/icons';
import { vTooltip } from '../../../shared/ui/kit';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon, modelLabel, type AcpQueuedPrompt, type Checkpoint, type Harness, type Subagent } from '../../../entities/session';
import type { ChatBlock } from '../model/chatBlocks';
import type { FeedNote } from '../model/feedNotes';
import { durationLabel } from '../model/duration';
import ToolGroup from './ToolGroup.vue';
import TurnFiles from './TurnFiles.vue';
import CheckpointRow from './CheckpointRow.vue';
import SubagentGroup from './SubagentGroup.vue';
import StoredAttachments from './StoredAttachments.vue';
import QueuedBubble from './QueuedBubble.vue';
import RewindAction from './RewindAction.vue';

const props = withDefaults(
  defineProps<{
    blocks: readonly ChatBlock[];
    harness: Harness | null;
    agentLabel: string;
    root: string | null;
    place?: string;
    panelId?: string;
    agentId?: string | null;
    checkpointOf?: ReadonlyMap<string, Checkpoint>;
    rewindOf?: ReadonlyMap<string, { seq: number; files: boolean }>;
    rewind?: (seq: number, text: string) => Promise<void>;
    spoken?: ReadonlySet<string>;
    subagentOf: ReadonlyMap<string, Subagent>;
    now: number;
    subagentsOpenable?: boolean;
    found?: string | null;
    notes?: readonly FeedNote[];
    workingSince?: number | null;
    queued?: readonly AcpQueuedPrompt[];
    canSendNow?: boolean;
  }>(),
  { place: '', panelId: undefined, agentId: null, checkpointOf: () => new Map(), rewindOf: () => new Map(), spoken: () => new Set(), subagentsOpenable: true, found: null, notes: () => [], workingSince: null, queued: () => [] },
);
const emit = defineEmits<{ restored: [];openSubagent: [callId: string]; editQueued: [id: number, text: string]; removeQueued: [id: number]; sendQueuedNow: [id: number] }>();

const { t, formatDate } = useI18n();
const notesAfter = (key: string) => props.notes.filter((note) => note.afterKey === key);
const working = computed(() => (props.workingSince === null ? null : durationLabel(Math.max(0, Math.round((props.now - props.workingSince) / 1000)))));
</script>

<template>
  <ol class="messages" data-testid="conversation-messages">
    <template v-for="block in blocks" :key="block.key">
      <li class="block" :data-block="block.type" :data-key="block.key" :data-found="block.key === found || undefined">
        <div v-if="block.type === 'prompt'" v-tooltip="block.message.at ? formatDate(block.message.at) : undefined" class="message" data-role="user" data-testid="conversation-message">
          <StoredAttachments v-if="block.message.attachments" :attachments="block.message.attachments" />
          <RichContent v-if="block.message.shell && block.message.markdown" class="shell" :data-shell="block.message.shell" :markdown="block.message.markdown" />
          <p v-else-if="block.message.text" class="user">{{ block.message.text }}</p>
          <span v-if="spoken.has(block.message.text.trim())" class="spoken" data-testid="conversation-spoken">{{ t('chat.sentByVoice') }}</span>
          <RewindAction v-if="rewind && rewindOf.get(block.message.key)" :seq="rewindOf.get(block.message.key)!.seq" :files="rewindOf.get(block.message.key)!.files" :agent-label="agentLabel" :apply="(seq) => rewind!(seq, block.message.text)" />
          <CheckpointRow v-if="agentId && checkpointOf.get(block.message.key)" :agent-id="agentId" :checkpoint="checkpointOf.get(block.message.key)!" :place="place" :agent-label="agentLabel" @restored="emit('restored')" />
        </div>
        <div v-else-if="block.type === 'turn'" v-tooltip="block.model ? modelLabel(block.model) : undefined" class="turn" data-testid="conversation-turn">
          <span class="tile" :data-harness="harness ?? 'other'">
            <HarnessIcon v-if="harness" :harness="harness" :size="11" />
            <Bot v-else :size="11" />
          </span>
          <span>{{ agentLabel }}</span>
          <span v-if="block.seconds !== null" class="turn-time">· {{ durationLabel(block.seconds) }}</span>
        </div>
        <ToolGroup v-else-if="block.type === 'tools'" :calls="block.calls" :seconds="block.seconds" :root="root" />
        <SubagentGroup v-else-if="block.type === 'subagents'" :agents="block.callIds.flatMap((callId) => subagentOf.get(callId) ?? [])" :now="now" :openable="subagentsOpenable" @open="emit('openSubagent', $event)" />
        <TurnFiles v-else-if="block.type === 'files'" :files="block.files" :root="root" :panel-id="panelId" />
        <div v-else v-tooltip="block.message.at ? formatDate(block.message.at) : undefined" class="message" :data-role="block.message.role" data-testid="conversation-message">
          <RichContent v-if="block.message.shell && block.message.markdown" class="shell" :data-shell="block.message.shell" :markdown="block.message.markdown" />
          <p v-else-if="block.message.role === 'user'" class="user">{{ block.message.text }}</p>
          <RichContent v-else class="assistant" :markdown="block.message.text" streaming />
        </div>
      </li>
      <li v-for="note in notesAfter(block.key)" :key="`note-${note.id}`" class="note" data-testid="conversation-note">
        <Check :size="13" />
        <span>{{ note.text }}</span>
        <code v-if="note.code">{{ note.code }}</code>
      </li>
    </template>
    <li v-if="working" class="note working" data-testid="conversation-working">
      <LoaderCircle :size="13" class="spin" />
      <span>{{ t('chat.working', { time: working }) }}</span>
    </li>
    <li v-for="item in queued" :key="`queued-${item.id}`" class="block queued" data-testid="conversation-queued">
      <QueuedBubble :item="item" :agent-label="agentLabel" :can-send-now="canSendNow" @edit="(id, text) => emit('editQueued', id, text)" @remove="emit('removeQueued', $event)" @send-now="emit('sendQueuedNow', $event)" />
    </li>
  </ol>
</template>

<style scoped>
.messages {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 0;
  margin-bottom: 0;
  padding: 0;
  list-style: none;
}

.messages :deep(.rich-content .body :is(p, ul, ol, dl, blockquote, h1, h2, h3, h4, h5, h6)) {
  max-width: none;
}

.block {
  line-height: 1.57;
}

.block[data-block='prompt'] {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  align-self: flex-end;
  max-width: 78%;
}

.block.queued {
  align-self: flex-end;
  max-width: 78%;
}

.block[data-block='turn'] {
  padding-top: var(--vr-space-1);
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

.tile {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  color: var(--vr-color-brand-contrast);
  background: var(--vr-color-agent-other);
  border-radius: var(--vr-radius-chip);
}

.tile[data-harness='claude'] {
  background: var(--vr-color-agent-claude);
}

.tile[data-harness='codex'] {
  background: var(--vr-color-agent-codex);
}

.tile[data-harness='cursor'] {
  background: var(--vr-color-agent-cursor);
}

.tile[data-harness] :deep(.harness) {
  color: var(--vr-color-brand-contrast);
}

.user {
  margin: 0;
  padding: 9px 14px;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-bubble) var(--vr-radius-bubble) var(--vr-radius-chip) var(--vr-radius-bubble);
}

.shell {
  font-size: var(--vr-font-size-xs);
}

.shell[data-shell='output'] {
  color: var(--vr-color-text-muted);
}

.assistant {
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  text-wrap: pretty;
}

.note {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
}

.note code {
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.working :deep(svg) {
  color: var(--vr-status-running);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
