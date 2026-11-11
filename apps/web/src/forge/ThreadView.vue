<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Check, CheckCircle2, ChevronRight, Link, MessageSquare } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import { describeError, forgeReply, forgeResolve, type ChangeRequestRef, type Discussion } from '../shared/api/connectors/api';
import { copyText } from '../shared/lib/atlassian';
import PersonAvatar from '../shared/ui/PersonAvatar.vue';
import RichContent from '../shared/ui/rich-content/RichContent.vue';
import { t } from '../shared/lib/i18n/i18n';
import CommentComposer from './CommentComposer.vue';
import { fullDate, relativeTime } from '../entities/merge-request/lib/format';

const props = defineProps<{ discussion: Discussion; connector?: string; showPosition?: boolean; target?: ChangeRequestRef }>();
const emit = defineEmits<{ goto: [discussion: Discussion]; update: [discussion: Discussion] }>();

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
  <article class="thread" :data-resolved="discussion.resolved" :data-thread-id="discussion.id" :data-unresolved="discussion.resolvable && !discussion.resolved" data-testid="thread">
    <button v-if="discussion.url" class="link" :title="copied ? t('forge.thread.copied') : t('forge.thread.copyLink')" :aria-label="t('forge.thread.copyLink')" data-testid="thread-copy-link" @click="copyLink">
      <Check v-if="copied" :size="13" />
      <Link v-else :size="13" />
    </button>
    <button v-if="discussion.resolved" class="fold" :aria-expanded="!folded" data-testid="thread-fold" @click="folded = !folded">
      <ChevronRight :size="14" class="vr-chevron" :data-open="!folded" />
      <CheckCircle2 :size="14" class="resolved-icon" />
      <span class="fold-text">{{ t('forge.thread.resolved') }}<template v-if="first"> · {{ first.author.name || first.author.username }}: {{ preview }}</template></span>
    </button>
    <template v-if="!folded">
      <button v-if="showPosition && position" class="position" :title="position" data-testid="thread-position" @click="emit('goto', discussion)">
        <MessageSquare :size="12" />{{ position }}
      </button>
      <div v-for="note in notes" :key="note.id" class="note" :data-system="note.system">
        <PersonAvatar v-if="!note.system" :person="note.author" small />
        <div class="body">
          <div class="meta">
            <b v-if="!note.system">{{ note.author.name || note.author.username }}</b>
            <span :title="fullDate(note.created_at)">{{ relativeTime(note.created_at) }}</span>
          </div>
          <p v-if="note.system" class="system">{{ note.body }}</p>
          <RichContent v-else :markdown="note.body" :connector="connector" />
        </div>
      </div>
      <template v-if="target">
        <CommentComposer
          v-if="replying"
          v-model="draft"
          testid="thread-reply"
          :busy="sending"
          :error="replyError"
          :placeholder="t('forge.composer.replyPlaceholder')"
          :label="t('forge.thread.send')"
          cancellable
          autofocus
          @send="sendReply"
          @cancel="replying = false"
        />
        <div v-if="!replying || discussion.resolvable" class="actions">
          <VButton v-if="!replying" size="small" severity="secondary" text :label="draft.trim() ? `${t('forge.thread.reply')} …` : t('forge.thread.reply')" data-testid="thread-reply" @click="replying = true" />
          <VButton v-if="discussion.resolvable" size="small" severity="secondary" text :label="discussion.resolved ? t('forge.thread.unresolve') : t('forge.thread.resolve')" :loading="resolving" data-testid="thread-resolve" @click="toggleResolved" />
        </div>
        <p v-if="resolveError" class="error" role="alert">{{ resolveError }}</p>
      </template>
    </template>
  </article>
</template>

<style scoped>
.thread {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-color-text);
  font-family: var(--vr-font-family);
  font-size: var(--vr-font-size-sm);
  white-space: normal;
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.link {
  position: absolute;
  top: var(--vr-space-2);
  right: var(--vr-space-2);
  display: inline-flex;
  padding: var(--vr-space-1);
  color: var(--vr-color-text-muted);
  background: none;
  border: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
  opacity: 0;
}

.thread:hover .link,
.link:focus-visible {
  opacity: 1;
}

.link:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
}

.thread[data-unresolved='true'] {
  border-left: 3px solid var(--vr-color-conflict);
}

.fold,
.position {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  align-self: flex-start;
  max-width: 100%;
  padding: 0;
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  background: none;
  border: none;
  cursor: pointer;
}

.position {
  font-family: var(--vr-font-mono);
}

.position:hover,
.fold:hover {
  color: var(--vr-color-text);
}

.fold-text {
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
  gap: var(--vr-space-2);
}

.body {
  flex: 1;
  min-width: 0;
}

.meta {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.meta b {
  color: var(--vr-color-text);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
}

.error {
  margin: 0;
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
  overflow-wrap: anywhere;
}

.system {
  margin: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-style: italic;
  overflow-wrap: anywhere;
}
</style>
