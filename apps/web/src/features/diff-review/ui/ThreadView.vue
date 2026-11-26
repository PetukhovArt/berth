<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Check, ChevronRight, Link, MessageSquare } from '../../../shared/ui/icons';
import { VButton, VInputText, VMessage, vTooltip } from '../../../shared/ui/kit';
import { describeError, forgeReply, forgeResolve, type ChangeRequestRef, type Discussion } from '../../../shared/api/connectors/api';
import type { PromptQuote } from '../../../entities/session';
import { copyText } from '../../../shared/lib/atlassian';
import PersonAvatar from '../../../shared/ui/PersonAvatar.vue';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import CommentComposer from './CommentComposer.vue';
import { fullDate, relativeTime } from '../../../entities/merge-request';

const props = defineProps<{ discussion: Discussion; connector?: string; showPosition?: boolean; target?: ChangeRequestRef; quotable?: boolean }>();
const emit = defineEmits<{ goto: [discussion: Discussion]; update: [discussion: Discussion]; quote: [quote: PromptQuote] }>();

const folded = shallowRef(props.discussion.resolved);
const copied = shallowRef(false);
const replying = shallowRef(false);
const draft = shallowRef('');
const sending = shallowRef(false);
const resolving = shallowRef(false);
const replyError = shallowRef<string | null>(null);
const resolveError = shallowRef<string | null>(null);
const notes = computed(() => props.discussion.notes);
const first = computed(() => notes.value.find((n) => !n.system) ?? notes.value[0]);
const preview = computed(() => (first.value?.body ?? '').replace(/\s+/g, ' ').slice(0, 120));
const position = computed(() => {
  const p = props.discussion.position;
  if (!p) return null;
  const line = p.new_line ?? p.old_line;
  return line == null ? p.path : `${p.path}:${line}`;
});

function quoteThread() {
  const comments = notes.value
    .filter((n) => !n.system)
    .map((n) => `> ${n.author.name || n.author.username}: ${n.body.trim()}`.replace(/\n/g, '\n> '));
  if (!comments.length) return;
  const label = position.value ?? t('forge.thread.quoteLabel', { name: first.value?.author.name || first.value?.author.username || '' });
  emit('quote', { label, text: `${position.value ? `${position.value}\n` : ''}${comments.join('\n')}\n` });
}

async function copyLink() {
  await copyText(props.discussion.url);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}

async function sendReply() {
  const target = props.target;
  const body = draft.value.trim();
  if (!target || !body || sending.value) return;
  sending.value = true;
  replyError.value = null;
  try {
    const updated = await forgeReply(target, props.discussion.id, body);
    draft.value = '';
    replying.value = false;
    emit('update', updated);
  } catch (e) {
    replyError.value = describeError(e);
  } finally {
    sending.value = false;
  }
}

async function toggleResolved() {
  const target = props.target;
  if (!target || resolving.value) return;
  resolving.value = true;
  resolveError.value = null;
  try {
    const updated = await forgeResolve(target, props.discussion.id, !props.discussion.resolved);
    folded.value = updated.resolved;
    emit('update', updated);
  } catch (e) {
    resolveError.value = describeError(e);
  } finally {
    resolving.value = false;
  }
}

function expand() {
  folded.value = false;
}

defineExpose({ expand });
</script>

<template>
  <article class="thread" :data-resolved="discussion.resolved" :data-folded="folded" :data-thread-id="discussion.id" :data-unresolved="discussion.resolvable && !discussion.resolved" data-testid="thread">
    <template v-if="folded">
      <Check :size="14" class="resolved-icon" />
      <VButton class="fold" size="small" severity="secondary" text :aria-expanded="false" data-testid="thread-fold" @click="folded = false">
        <span class="fold-text">{{ t('forge.thread.resolved') }}<template v-if="first"> · {{ first.author.name || first.author.username }}: {{ preview }}</template></span>
      </VButton>
      <VButton v-if="target && discussion.resolvable" size="small" severity="secondary" outlined :label="t('forge.thread.unresolve')" :loading="resolving" data-testid="thread-resolve" @click="toggleResolved" />
      <VMessage v-if="resolveError" severity="error" size="small">{{ resolveError }}</VMessage>
    </template>
    <template v-else>
      <VButton v-if="discussion.resolved" class="fold head" size="small" severity="secondary" text :aria-expanded="true" data-testid="thread-fold" @click="folded = true">
        <ChevronRight :size="14" class="vr-chevron" data-open="true" />
        <Check :size="14" class="resolved-icon" />
        <span class="fold-text">{{ t('forge.thread.resolved') }}<template v-if="first"> · {{ first.author.name || first.author.username }}: {{ preview }}</template></span>
      </VButton>
      <VButton v-if="showPosition && position" v-tooltip="position" class="position" size="small" severity="secondary" text data-testid="thread-position" @click="emit('goto', discussion)">
        <MessageSquare :size="12" />{{ position }}
      </VButton>
      <div v-for="note in notes" :key="note.id" class="note" :data-system="note.system">
        <PersonAvatar v-if="!note.system" :person="note.author" small />
        <div class="body">
          <div class="meta">
            <b v-if="!note.system">{{ note.author.name || note.author.username }}</b>
            <span v-tooltip="fullDate(note.created_at)">{{ relativeTime(note.created_at) }}</span>
          </div>
          <p v-if="note.system" class="system">{{ note.body }}</p>
          <RichContent v-else :markdown="note.body" :connector="connector" />
        </div>
      </div>
      <div v-if="target || quotable || discussion.url" class="foot">
        <template v-if="target">
          <CommentComposer
            v-if="replying"
            v-model="draft"
            testid="thread-reply"
            :busy="sending"
            :error="replyError"
            :placeholder="t('forge.composer.replyPlaceholder')"
            :label="t('forge.composer.comment')"
            cancellable
            autofocus
            @send="sendReply"
            @cancel="replying = false"
          />
          <div v-else class="idle">
            <VInputText class="reply-field" size="small" readonly :model-value="''" :placeholder="draft.trim() ? `${t('forge.thread.reply')} …` : `${t('forge.thread.reply')}…`" :aria-label="t('forge.thread.reply')" data-testid="thread-reply" @focus="replying = true" @click="replying = true" />
            <VButton v-if="discussion.resolvable" size="small" severity="secondary" outlined :label="discussion.resolved ? t('forge.thread.unresolve') : t('forge.thread.resolve')" :loading="resolving" data-testid="thread-resolve" @click="toggleResolved">
              <template v-if="!discussion.resolved" #icon><Check :size="14" class="resolved-icon" /></template>
            </VButton>
          </div>
          <VMessage v-if="resolveError" severity="error" size="small">{{ resolveError }}</VMessage>
        </template>
        <div v-if="quotable || discussion.url" class="actions">
          <span class="spacer" />
          <VButton v-if="quotable" v-tooltip="t('forge.thread.addToPromptTitle')" size="small" severity="secondary" text :label="t('forge.thread.addToPrompt')" data-testid="thread-add-to-prompt" @click="quoteThread" />
          <VButton v-if="discussion.url" v-tooltip="copied ? t('forge.thread.copied') : t('forge.thread.copyLink')" size="small" severity="secondary" text :aria-label="t('forge.thread.copyLink')" data-testid="thread-copy-link" @click="copyLink">
            <template #icon><Check v-if="copied" :size="13" /><Link v-else :size="13" /></template>
          </VButton>
        </div>
      </div>
    </template>
  </article>
</template>

<style scoped>
.thread {
  position: relative;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--vr-color-text);
  font-family: var(--vr-font-family);
  font-size: var(--vr-font-size-label);
  white-space: normal;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-md);
}

.thread[data-folded='true'] {
  flex-direction: row;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  min-height: 40px;
  box-sizing: border-box;
  padding: 6px var(--vr-space-2) 6px var(--vr-space-3);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-sunken);
  border-color: var(--vr-color-border);
}

.fold,
.position {
  justify-content: flex-start;
  align-self: flex-start;
  max-width: 100%;
  font-size: var(--vr-font-size-xs);
}

.fold {
  flex: 1;
  min-width: 0;
}

.fold.head {
  flex: none;
  align-self: stretch;
  padding: var(--vr-space-2) var(--vr-space-3);
  background: var(--vr-color-surface-sunken);
  border-bottom: 1px solid var(--vr-color-border);
}

.position {
  align-self: stretch;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-family: var(--vr-font-mono);
  background: var(--vr-color-surface-sunken);
  border-bottom: 1px solid var(--vr-color-border);
}

.fold-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.resolved-icon {
  flex: none;
  color: var(--vr-color-success);
}

.note {
  display: flex;
  gap: 10px;
  padding: var(--vr-space-3) 14px;
}

.note + .note {
  border-top: 1px solid var(--vr-color-border);
}

.body {
  flex: 1;
  min-width: 0;
}

.meta {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.meta b {
  color: var(--vr-color-text);
}

.foot {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: 10px var(--vr-space-3);
  background: var(--vr-color-surface-sunken);
  border-top: 1px solid var(--vr-color-border);
}

.idle {
  display: flex;
  gap: 10px;
  align-items: center;
}

.reply-field {
  flex: 1;
  min-width: 0;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.spacer {
  flex: 1;
}

.system {
  margin: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-style: italic;
  overflow-wrap: anywhere;
}
</style>
