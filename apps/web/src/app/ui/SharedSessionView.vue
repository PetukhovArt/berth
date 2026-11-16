<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue';
import { remoteFetch } from '../../bridge';
import { Eye, FileText, GitBranch, Wrench } from '../../shared/ui/icons';
import RichContent from '../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../shared/lib/i18n/i18n';
import { acpMessages, withShellBlocks, type AcpItemAt, type TranscriptMessage } from '../../entities/session';

type Status = 'fresh' | 'running' | 'finished' | 'needs_feedback' | 'terminated' | 'disconnected';
type Change = { path: string; status: string; additions: number; deletions: number };
type Feed = {
  name: string;
  status: Status;
  branch: string | null;
  messages: { transcript: TranscriptMessage[] } | { acp: AcpItemAt[] };
  changes: Change[];
};
type Block = { key: string; tools: TranscriptMessage[] } | { key: string; message: TranscriptMessage };
type State = 'connecting' | 'live' | 'gone' | 'ended';

const POLL_MS = 3000;
const PIN_PX = 48;

const feed = shallowRef<Feed | null>(null);
const state = shallowRef<State>('connecting');
const scroller = useTemplateRef<HTMLElement>('scroller');
let timer: ReturnType<typeof setTimeout> | undefined;
let stopped = false;

const messages = computed<TranscriptMessage[]>(() => {
  const source = feed.value?.messages;
  if (!source) return [];
  return 'acp' in source ? acpMessages(source.acp) : withShellBlocks(source.transcript);
});

const blocks = computed<Block[]>(() => {
  const out: Block[] = [];
  for (const message of messages.value) {
    const last = out.at(-1);
    if (message.tool && last && 'tools' in last) last.tools.push(message);
    else if (message.tool) out.push({ key: message.key, tools: [message] });
    else out.push({ key: message.key, message });
  }
  return out;
});

async function poll() {
  try {
    const response = await remoteFetch('/__berth/share');
    if (response.status === 401 || response.status === 403) state.value = 'ended';
    else if (response.status === 410) state.value = 'gone';
    else if (response.ok) {
      const el = scroller.value;
      const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < PIN_PX;
      feed.value = (await response.json()) as Feed;
      state.value = 'live';
      if (pinned) {
        await nextTick();
        const box = scroller.value;
        if (box) box.scrollTop = box.scrollHeight;
      }
    }
  } catch {
    state.value = feed.value ? 'live' : 'connecting';
  }
  if (stopped || state.value === 'ended' || state.value === 'gone') return;
  timer = setTimeout(() => void poll(), POLL_MS);
}

onMounted(() => {
  if (location.search) history.replaceState(null, '', location.pathname);
  void poll();
});

onBeforeUnmount(() => {
  stopped = true;
  clearTimeout(timer);
});
</script>

<template>
  <main class="shared" data-testid="shared-session">
    <header class="head">
      <h1 class="title" data-testid="shared-session-title">{{ feed?.name ?? 'Berth' }}</h1>
      <span v-if="feed" class="chip" data-testid="shared-session-status" :data-status="feed.status">{{ t(`agents.status.${feed.status}`) }}</span>
      <span v-if="feed?.branch" class="chip mono"><GitBranch :size="12" />{{ feed.branch }}</span>
      <span class="chip"><Eye :size="12" />{{ t('remote.share.readOnly') }}</span>
    </header>
    <p v-if="state === 'ended'" class="state" role="alert" data-testid="shared-session-ended">{{ t('remote.share.ended') }}</p>
    <p v-else-if="state === 'gone'" class="state" role="alert" data-testid="shared-session-gone">{{ t('remote.share.gone') }}</p>
    <p v-else-if="state === 'connecting'" class="state">{{ t('remote.share.connecting') }}</p>
    <div v-else class="body">
      <div ref="scroller" class="scroll" data-testid="shared-session-scroll">
        <p v-if="!blocks.length" class="state">{{ t('remote.share.empty') }}</p>
        <ol v-else class="messages" data-testid="shared-session-messages">
          <li v-for="block in blocks" :key="block.key" class="message" :data-role="'message' in block ? block.message.role : 'assistant'">
            <details v-if="'tools' in block" class="tools" data-testid="shared-session-tools">
              <summary class="tool"><Wrench :size="12" />{{ t('remote.share.toolCalls', { count: block.tools.length }) }}</summary>
              <div v-for="tool in block.tools" :key="tool.key" class="tool">
                <span class="tool-name">{{ tool.tool }}</span>
                <span class="tool-arg mono">{{ tool.text }}</span>
              </div>
            </details>
            <RichContent v-else-if="block.message.shell && block.message.markdown" class="shell" :data-shell="block.message.shell" :markdown="block.message.markdown" />
            <p v-else-if="block.message.role === 'user'" class="user">{{ block.message.text }}</p>
            <RichContent v-else class="assistant" :markdown="block.message.text" />
          </li>
        </ol>
      </div>
      <aside class="changes" data-testid="shared-session-changes">
        <h2 class="changes-title">{{ t('remote.share.changes') }}</h2>
        <p v-if="!feed?.changes.length" class="state">{{ t('remote.share.noChanges') }}</p>
        <ul v-else class="files">
          <li v-for="change in feed.changes" :key="change.path" class="file" :data-status="change.status">
            <FileText :size="12" />
            <span class="path mono" :title="change.path">{{ change.path }}</span>
            <span class="added">+{{ change.additions }}</span>
            <span class="removed">-{{ change.deletions }}</span>
          </li>
        </ul>
      </aside>
    </div>
  </main>
</template>

<style scoped>
.shared {
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 0;
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
}

.head {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 2px var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.scroll {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-6);
}

.changes {
  flex: none;
  width: 280px;
  overflow-y: auto;
  padding: var(--vr-space-3);
  border-left: 1px solid var(--vr-color-border);
}

:root[data-vr-viewport='phone'] .body {
  flex-direction: column;
}

:root[data-vr-viewport='phone'] .changes {
  width: auto;
  max-height: 30%;
  border-top: 1px solid var(--vr-color-border);
  border-left: 0;
}

.changes-title {
  margin: 0 0 var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.files {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.file {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
}

.path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  direction: rtl;
  text-align: left;
}

.added {
  color: var(--vr-color-success);
}

.removed {
  color: var(--vr-color-danger);
}

.state {
  margin: var(--vr-space-3) var(--vr-space-4);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.messages {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 920px;
  margin: 0 auto;
  padding: 0;
  list-style: none;
}

.message[data-role='user'] {
  align-self: flex-end;
  max-width: 85%;
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

.tools summary {
  cursor: pointer;
}

.tool {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  padding: 2px var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.tool-name {
  flex: none;
  font-weight: 600;
}

.tool-arg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
