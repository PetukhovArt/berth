<script setup lang="ts">
import { computed } from 'vue';
import { Eye } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { useConnectors, type ChangeRequest, type Discussion } from '../../../shared/api/connectors/api';
import PersonAvatar from '../../../shared/ui/PersonAvatar.vue';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { CommentComposer, ThreadView } from '../../../features/diff-review';
import { issueKeyOf, openIssue } from '../../../entities/jira-issue';
import { numberLabel, pipelineLabel } from '../../../entities/merge-request';
import { StatusIcon, useSessionDock, useSessionStates, type PromptQuote, type SessionNode } from '../../../entities/session';
import { isWatching, toggleWatch } from '../../../features/watch-merge-request';
import { GitStepButton } from '../../../features/git-step';
import { useMrWorktree } from '../../../features/open-in-session';
import { useChangeRequest } from '../model/change-request-scope';
import { pipelineTone } from '../model/pipelineTone';
import PipelineJobs from './PipelineJobs.vue';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ connector: string; request: ChangeRequest; session: SessionNode | null }>();
const emit = defineEmits<{ goto: [discussion: Discussion]; quote: [quote: PromptQuote] }>();

const dock = useSessionDock();
const states = useSessionStates();
const statusOf = (session: SessionNode) => states.value.get(session.key)?.status ?? session.status;
const watching = computed(() => isWatching(target.value));
const number = computed(() => numberLabel(props.request));

const { target, discussions, pipeline, commentDraft, commenting, commentError, selectTab, upsertDiscussion, sendComment } = useChangeRequest();

const { connectors } = useConnectors();
const jiraConnector = computed(() => connectors.value.find((c) => c.kind === 'jira'));
const issueKey = computed(() => issueKeyOf(props.request.source_branch, props.request.title, props.request.description));

const worktree = useMrWorktree(() => target.value);
const unresolved = computed(() => discussions.value.filter((d) => d.resolvable && !d.resolved).length);
const resolvable = computed(() => discussions.value.some((d) => d.resolvable));
const conversation = computed(() => discussions.value.filter((d) => d.notes.some((n) => !n.system)));
const anchored = computed(() => discussions.value.some((d) => d.position));
const pipelineStatus = computed(() => (pipeline.value ? pipeline.value.status : props.request.pipeline));
</script>

<template>
  <div class="overview" data-testid="cr-overview">
    <div class="layout">
      <div class="threads-col">
        <RichContent v-if="props.request.description.trim()" :markdown="props.request.description" :connector="props.connector" variant="document" />
        <p v-else class="muted subtle">{{ t('forge.request.noDescription') }}</p>

        <div v-if="conversation.length" class="discussions-head">
          <h3 class="section">{{ t('forge.request.discussions') }} <span class="count">{{ conversation.length }}</span></h3>
          <span v-if="resolvable" class="pill" data-testid="cr-unresolved-pill">{{ unresolved ? t('forge.request.unresolvedPill', { count: unresolved }) : t('forge.request.allResolved') }}</span>
          <span v-if="anchored" class="hint vr-key-hint">{{ t('forge.request.keysHint') }}</span>
        </div>
        <div class="threads">
          <ThreadView v-for="d in conversation" :key="d.id" :discussion="d" :connector="props.connector" :target="target" :quotable="!!session" show-position @goto="emit('goto', $event)" @update="upsertDiscussion" @quote="emit('quote', $event)" />
        </div>
        <CommentComposer
          v-model="commentDraft"
          testid="cr-comment"
          :busy="commenting"
          :error="commentError"
          :placeholder="t('forge.composer.commentPlaceholder')"
          :label="t('forge.composer.comment')"
          @send="sendComment"
          @cancel="commentError = null"
        />
      </div>

      <aside class="facts" data-testid="cr-facts">
        <section class="fact" data-testid="cr-fact-pipeline">
          <h3 class="fact-title">{{ pipeline ? t('forge.request.pipelineNumber', { id: pipeline.id }) : t('forge.request.tabPipeline') }}</h3>
          <UiButton class="status-link" @click="selectTab('pipeline')">
            <StatusIcon v-if="pipelineStatus !== 'none'" :tone="pipelineTone(pipelineStatus)" />
            <span>{{ pipelineLabel(pipelineStatus) }}</span>
          </UiButton>
          <PipelineJobs v-if="pipeline?.jobs.length" compact />
        </section>

        <section v-if="props.request.reviewers.length" class="fact" data-testid="cr-fact-reviewers">
          <h3 class="fact-title">{{ t('forge.request.reviewersHeading') }}</h3>
          <div v-for="r in props.request.reviewers" :key="r.username" class="reviewer">
            <PersonAvatar :person="r" small />
            <span class="reviewer-name">{{ r.name || r.username }}</span>
          </div>
        </section>

        <section v-if="issueKey" class="fact" data-testid="cr-fact-issue">
          <h3 class="fact-title">{{ t('forge.request.issueHeading') }}</h3>
          <UiButton v-if="jiraConnector" class="status-link issue-key" :title="t('forge.request.openIssue', { key: issueKey })" @click="openIssue(jiraConnector.id, issueKey)">{{ issueKey }}</UiButton>
          <span v-else class="issue-key">{{ issueKey }}</span>
        </section>

        <section class="fact" data-testid="cr-fact-session">
          <h3 class="fact-title">{{ t('forge.request.sessionHeading') }}</h3>
          <UiButton v-if="session" class="status-link session-link" :title="t('forge.request.openSession')" data-testid="cr-fact-session-open" @click="dock.openWhenListed(session.ref)">
            <StatusIcon :status="statusOf(session)" :unseen="session.unseen" />
            <span class="session-name">{{ session.name }}</span>
            <span class="pipe-id">{{ t(`agents.status.${statusOf(session)}`) }}</span>
          </UiButton>
          <span v-else class="pipe-id" data-testid="cr-fact-no-session">{{ t('forge.request.noSession') }}</span>
          <template v-if="request.state === 'open'">
            <VButton size="small" severity="secondary" :label="watching ? t('attention.watch.stopWatching') : `${t('attention.watch.action')} ${number}`" :aria-pressed="watching" data-testid="cr-fact-watch" @click="toggleWatch(target)">
              <template #icon><Eye :size="14" /></template>
            </VButton>
            <p class="pipe-id hint-text">{{ session ? t('attention.watch.idleNamed', { name: session.name }) : t('attention.watch.idle') }}</p>
          </template>
        </section>

        <section v-if="request.state === 'open' && worktree?.branch" class="fact next-step" data-testid="cr-fact-next-step">
          <h3 class="fact-title">{{ t('forge.request.nextStepHeading') }}</h3>
          <GitStepButton testid="cr-git-step" :path="worktree.path" :branch="worktree.branch" has-request :refresh-key="request.head_sha" />
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.overview {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  align-items: start;
  gap: var(--vr-space-6);
  padding: var(--vr-space-5) var(--vr-space-5) var(--vr-space-6);
}

:root[data-vr-viewport='phone'] .layout {
  grid-template-columns: minmax(0, 1fr);
}

.threads-col {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
  min-width: 0;
}

.muted {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.muted.subtle {
  color: var(--vr-color-text-subtle);
}

.discussions-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.section {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.count {
  color: var(--vr-color-text-muted);
  font-weight: 400;
}

.pill {
  display: inline-flex;
  align-items: center;
  height: 20px;
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-full);
}

.next-step :deep(.git-step) {
  flex-direction: column;
  align-items: stretch;
}

.next-step :deep(.git-step > .hint) {
  order: 1;
  white-space: normal;
}

.next-step :deep(.git-step > .form),
.next-step :deep(.git-step > .error) {
  position: static;
  width: auto;
  max-width: none;
}

.next-step :deep(.git-step .actions) {
  flex-wrap: wrap;
}

.hint {
  margin-left: auto;
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.threads {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
}

.facts {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-xl);
}

:root[data-vr-viewport='phone'] .facts {
  position: static;
}

.fact {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4);
}

.fact + .fact {
  border-top: 1px solid var(--vr-color-border);
}

.fact-title {
  margin: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
}

.status-link {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  align-self: flex-start;
  padding: 0;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  background: none;
  border: none;
  cursor: pointer;
}

.issue-key {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.pipe-id {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.hint-text {
  margin: 0;
}

.session-link {
  max-width: 100%;
}

.session-name {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.fact :deep(button[data-testid='cr-fact-watch']) {
  align-self: flex-start;
}

.reviewer {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
}

.reviewer-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
