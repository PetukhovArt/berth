<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { ArrowRight, ExternalLink, GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft, RefreshCw } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { forgeChangeRequest, openUrl, useConnectors, type ChangeRequest, type Discussion } from '../../../shared/api/connectors/api';
import PersonAvatar from '../../../shared/ui/PersonAvatar.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import { ArchitectureAction } from '../../../features/architecture';
import { MrSessionNotice, OpenInSessionAction, mrRuns, useMrSession } from '../../../features/open-in-session';
import { quoteToPrompt } from '../../../features/session-view-switch';
import { OpenChannelAction } from '../../../features/attention';
import { channelKeyOf } from '../../../entities/jira-issue';
import { WatchAction, WatchStrip, isWatching } from '../../../features/watch-merge-request';
import { RepoActions } from '../../../features/manage-workspace';
import { itemId, fullDate, numberLabel, pipelineLabel, relativeTime } from '../../../entities/merge-request';
import { StatusIcon, type PromptQuote } from '../../../entities/session';
import { DiffView } from '../../../features/diff-review';
import { createChangeRequest } from '../model/changeRequest';
import { pipelineTone } from '../model/pipelineTone';
import { provideChangeRequest } from '../model/change-request-scope';
import PipelineJobs from './PipelineJobs.vue';
import ChangeRequestOverview from './ChangeRequestOverview.vue';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ connector: string; project: string; number: number; panel?: DockviewPanelApi }>();

const STATE_KEY = { open: 'forge.request.state.open', merged: 'forge.request.state.merged', closed: 'forge.request.state.closed' } as const;

const scope = useScope();
const store = createChangeRequest(() => ({ connector: props.connector, project: props.project, number: props.number }), scope);
provideChangeRequest(store);
const { target, cr, discussions, pipeline, files, loading, failure, discussionsFailure, pipelineFailure, diffLoading, diffFailure, diffLoaded, tab, pipelineOpen, generation, load, selectTab } = store;
const diffRef = useTemplateRef<InstanceType<typeof DiffView>>('diff');

const { connectors } = useConnectors();
const requestLabel = computed(() => {
  if (cr.value) return numberLabel(cr.value);
  const kind = connectors.value.find((c) => c.id === props.connector)?.kind;
  return `${kind === 'gitlab' ? '!' : '#'}${props.number}`;
});

const unresolved = computed(() => discussions.value.filter((d) => d.resolvable && !d.resolved).length);
const sessionRun = computed(() => mrRuns.value.get(itemId(target.value)));
const pipelineStatus = computed(() => (pipeline.value ? pipeline.value.status : (cr.value?.pipeline ?? 'none')));

const session = useMrSession(() => target.value);

function quote(entry: PromptQuote) {
  if (session.value) quoteToPrompt(session.value.ref, entry, props.panel?.id === itemId(target.value) ? props.panel.id : undefined);
}

const diffState = computed(() => {
  if (diffFailure.value) return { text: diffFailure.value, testid: 'cr-diff-failure', alert: true };
  if (diffLoading.value && !diffLoaded.value) return { text: t('forge.request.loadingChanges'), testid: 'cr-diff-loading' };
  return null;
});

const watching = computed(() => isWatching(target.value));
const pendingThread =shallowRef<string | null>(null);

function gotoThread(discussion: Discussion) {
  pendingThread.value = discussion.id;
  selectTab('diff');
}

watch([diffLoaded, tab, pendingThread, diffRef], async () => {
  const id = pendingThread.value;
  if (!id || tab.value !== 'diff' || !diffLoaded.value || !diffRef.value) return;
  pendingThread.value = null;
  await nextTick();
  diffRef.value?.focusThread(id);
});

const POLL_MS = 30_000;
const fingerprint = (request: ChangeRequest) => [request.updated_at, request.head_sha, request.pipeline, request.unresolved_threads, request.state].join('|');
let missed = false;

async function poll() {
  if (document.visibilityState !== 'visible' || loading.value || !cr.value) return;
  const ticket = generation();
  const fresh = await forgeChangeRequest(target.value).catch(() => null);
  if (fresh && ticket === generation() && !loading.value && cr.value && fingerprint(fresh) !== fingerprint(cr.value)) void load();
}

function tick() {
  if (props.panel && !props.panel.isVisible) missed = true;
  else void poll();
}

onMounted(() => {
  void load();
  visibleInterval(scope, POLL_MS, tick);
  const shown = props.panel?.onDidVisibilityChange((event) => {
    if (!event.isVisible || !missed) return;
    missed = false;
    void poll();
  });
  if (shown) scope.add(() => shown.dispose());
});
</script>

<template>
  <section class="cr" data-testid="change-request">
    <p v-if="failure" class="notice failure" role="alert" data-testid="cr-failure">
      <StatusIcon tone="feedback" />
      <span class="notice-text">{{ t('forge.request.loadFailed', { number: requestLabel, reason: failure }) }}</span>
      <UiButton class="link-button" type="button" data-testid="cr-failure-refresh" @click="load">{{ t('common.refresh') }}</UiButton>
    </p>
    <div v-else-if="!cr" class="skeleton" aria-busy="true" data-testid="cr-loading">
      <span class="loading">{{ t('common.loading') }}</span>
      <i class="bar title-bar" />
      <i class="bar sub-bar" />
      <i class="bar gap" />
      <i class="bar row-bar" />
      <i class="bar row-bar" />
      <i class="bar row-bar" />
    </div>

    <template v-if="cr">
      <header class="head" :data-compact="tab !== 'overview'">
        <div class="title-row">
          <component :is="cr.state === 'merged' ? GitMerge : cr.state === 'closed' ? GitPullRequestClosed : cr.draft ? GitPullRequestDraft : GitPullRequest" :size="16" class="state-icon" :data-state="cr.state" />
          <h2 class="title">{{ cr.title }} <span class="num">{{ numberLabel(cr) }}</span></h2>
          <div class="actions">
            <VButton size="small" severity="secondary" text :loading="loading" :aria-label="t('common.refresh')" :title="t('common.refresh')" data-testid="cr-refresh" @click="load">
              <template #icon><RefreshCw :size="14" /></template>
            </VButton>
            <VButton size="small" severity="secondary" :label="t('common.openInBrowser')" :title="t('forge.request.openInBrowserTitle')" data-testid="cr-open-browser" @click="openUrl(cr.url)">
              <template #icon><ExternalLink :size="14" /></template>
            </VButton>
            <OpenInSessionAction :item="cr" />
            <OpenChannelAction :channel-key="channelKeyOf(cr.source_branch, cr.title)" />
            <ArchitectureAction :item="cr" />
            <WatchAction v-if="cr.state === 'open'" :target="target" />
          </div>
        </div>

        <div class="meta">
          <span class="chip state" :data-state="cr.state" data-testid="cr-state">{{ t(STATE_KEY[cr.state]) }}</span>
          <span v-if="cr.draft" class="chip" data-testid="cr-draft">{{ t('forge.draft') }}</span>
          <span v-if="cr.state === 'open' && cr.mergeable === false" class="chip warn" :title="t('forge.request.conflictsTitle')" data-testid="cr-conflicts">{{ t('forge.request.conflicts') }}</span>
          <span class="branches" :title="`${cr.source_branch} → ${cr.target_branch}`">
            <code>{{ cr.source_branch }}</code><ArrowRight :size="12" /><code>{{ cr.target_branch }}</code>
          </span>
          <span aria-hidden="true">·</span>
          <span class="person"><PersonAvatar :person="cr.author" small />{{ cr.author.name || cr.author.username }}</span>
          <span aria-hidden="true">·</span>
          <span :title="fullDate(cr.updated_at)">{{ t('forge.request.updated', { time: relativeTime(cr.updated_at) }) }}</span>
        </div>

        <div class="meta">
          <span v-if="cr.reviewers.length" class="person reviewers" data-testid="cr-reviewers">
            <span>{{ t('forge.request.reviewers') }}</span>
            <span v-for="r in cr.reviewers" :key="r.username" class="person" data-testid="cr-reviewer"><PersonAvatar :person="r" small />{{ r.name || r.username }}</span>
          </span>
          <span v-for="label in cr.labels" :key="label" class="chip label">{{ label }}</span>
          <UiButton v-if="pipeline" class="chip pipeline" :aria-expanded="pipelineOpen" data-testid="cr-pipeline-chip" @click="pipelineOpen = !pipelineOpen">
            <StatusIcon v-if="pipelineStatus !== 'none'" :tone="pipelineTone(pipelineStatus)" />{{ pipelineLabel(pipelineStatus) }}
          </UiButton>
          <span v-else class="chip pipeline" data-testid="cr-pipeline-chip"><StatusIcon v-if="pipelineStatus !== 'none'" :tone="pipelineTone(pipelineStatus)" />{{ pipelineLabel(pipelineStatus) }}</span>
        </div>

        <div v-if="pipeline && pipelineOpen" class="pipeline-pop" data-testid="cr-pipeline-jobs">
          <a class="pop-title" :href="pipeline.url" @click.prevent="openUrl(pipeline.url)">{{ t('forge.request.pipelineNumber', { id: pipeline.id }) }} <ExternalLink :size="12" /></a>
          <PipelineJobs compact />
        </div>
        <WatchStrip v-if="watching && cr.state === 'open'" :target="target" :url="cr.url" />
        <p v-if="discussionsFailure" class="notice" role="alert" :title="discussionsFailure" data-testid="cr-discussions-failure">
          <StatusIcon tone="feedback" /><span>{{ t('forge.request.discussionsFailed') }}</span>
        </p>
        <p v-if="pipelineFailure" class="notice" role="alert" :title="pipelineFailure" data-testid="cr-pipeline-failure">
          <StatusIcon tone="feedback" /><span>{{ t('forge.request.pipelineFailed') }}</span>
        </p>
        <MrSessionNotice v-if="sessionRun" :run="sessionRun">
          <template #project="{ repo }"><RepoActions :repo="repo" /></template>
        </MrSessionNotice>
      </header>

      <nav class="tabs" role="tablist">
        <UiButton class="tab" role="tab" :aria-selected="tab === 'overview'" data-testid="cr-tab-overview" @click="selectTab('overview')">
          {{ t('forge.request.tabOverview') }}<span v-if="unresolved" class="count" :title="t('forge.unresolvedThreads', { count: unresolved })">{{ unresolved }}</span>
        </UiButton>
        <UiButton class="tab" role="tab" :aria-selected="tab === 'diff'" data-testid="cr-tab-diff" @click="selectTab('diff')">
          {{ t('forge.request.tabChanges') }}<span v-if="diffLoaded" class="count">{{ files.length }}</span>
        </UiButton>
        <UiButton class="tab" role="tab" :aria-selected="tab === 'pipeline'" data-testid="cr-tab-pipeline" @click="selectTab('pipeline')">{{ t('forge.request.tabPipeline') }}</UiButton>
      </nav>

      <ChangeRequestOverview v-show="tab === 'overview'" :connector="connector" :request="cr" :session="session" @goto="gotoThread" @quote="quote" />

      <div v-if="tab === 'diff' || diffLoaded" v-show="tab === 'diff'" class="pane" data-testid="cr-diff">
        <DiffView ref="diff" :files="files" :discussions="discussions" :connector="connector" :project="project" :head-sha="cr.head_sha" :request-url="cr.url" :quotable="!!session" :state="diffState" @quote="quote">
          <template #scope>
            <span class="versus">{{ t('forge.diff.versus', { base: cr.target_branch }) }}</span>
          </template>
        </DiffView>
      </div>

      <div v-show="tab === 'pipeline'" class="pane scroll" data-testid="cr-pipeline">
        <div class="pipeline-body">
          <template v-if="pipeline?.jobs.length">
            <p class="pipeline-head">
              <StatusIcon :tone="pipelineTone(pipeline.status)" />
              <b data-testid="cr-pipeline-title">{{ t('forge.request.pipelineNumber', { id: pipeline.id }) }}</b>
              <span class="muted">{{ pipelineLabel(pipeline.status) }}</span>
              <a class="link open-link" :href="pipeline.url" data-testid="cr-pipeline-open" @click.prevent="openUrl(pipeline.url)">{{ t('common.open') }}</a>
            </p>
            <PipelineJobs />
          </template>
          <p v-else-if="pipeline" class="empty">{{ t('forge.pipelineJobs.empty') }}</p>
          <p v-else class="empty">{{ t('forge.request.noPipeline') }}</p>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.cr {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.loading,
.muted {
  color: var(--vr-color-text-muted);
}

.loading,
.failure {
  margin: var(--vr-space-4);
}

.failure {
  color: var(--vr-color-danger);
}

.versus {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
}

.notice {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-xs);
}

.notice.failure {
  margin: var(--vr-space-4) var(--vr-space-5);
  padding: var(--vr-space-3);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-lg);
}

.notice-text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}

.link-button {
  padding: 0;
  color: var(--vr-color-link);
  font: inherit;
  background: none;
  border: none;
  cursor: pointer;
}

.skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  padding: var(--vr-space-4) var(--vr-space-5);
}

.skeleton .loading {
  margin: 0;
  font-size: var(--vr-font-size-xs);
}

.bar {
  display: block;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.title-bar {
  width: 44%;
  height: 18px;
}

.sub-bar {
  width: 30%;
  height: 12px;
}

.bar.gap {
  height: 6px;
  background: none;
}

.row-bar {
  height: 40px;
  border-radius: var(--vr-radius-md);
}

.head {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-5) var(--vr-space-2);
}

:root[data-vr-viewport='phone'] .head[data-compact='true'] :is(.meta, .actions) {
  display: none;
}

:root[data-vr-viewport='phone'] .head[data-compact='true'] .title {
  overflow: hidden;
  font-size: var(--vr-font-size-base);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.title-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.title {
  flex: 1 1 16rem;
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  margin-left: auto;
}

.actions :deep(button) {
  white-space: nowrap;
}

.num {
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-weight: 400;
}

.state-icon {
  flex: none;
  color: var(--vr-status-running);
}

.state-icon[data-state='merged'] {
  color: var(--vr-color-text);
}

.state-icon[data-state='closed'] {
  color: var(--vr-color-text-muted);
}

.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 22px;
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border: none;
  border-radius: var(--vr-radius-full);
}

.chip.state[data-state='open'] {
  color: var(--vr-status-running);
  background: var(--vr-tint-running);
}

.chip.state[data-state='merged'] {
  color: var(--vr-color-text);
}

.chip.warn {
  color: var(--vr-status-feedback);
  background: var(--vr-tint-feedback);
}

.chip.label {
  color: var(--vr-color-text);
}

button.chip {
  color: var(--vr-color-text);
  cursor: pointer;
}

.chip.pipeline {
  color: var(--vr-color-text);
}

.branches {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  color: var(--vr-color-text-muted);
}

code {
  font-family: var(--vr-font-mono);
  color: inherit;
}

.person {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.pipeline-pop {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  width: 360px;
  max-width: 100%;
  max-height: 260px;
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-2);
  overflow: auto;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.pop-title {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  align-self: flex-start;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
}

.pop-title:hover {
  text-decoration: underline;
}

.link {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  align-self: flex-start;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-xs);
  text-decoration: underline;
  cursor: pointer;
}

.tabs {
  display: flex;
  flex: none;
  gap: 18px;
  height: 34px;
  padding: 0 var(--vr-space-5);
  border-bottom: 1px solid var(--vr-color-border);
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: 0;
  color: var(--vr-color-text-muted);
  font: inherit;
  background: none;
  border: none;
  box-shadow: none;
  cursor: pointer;
}

.tab[aria-selected='true'] {
  color: var(--vr-color-text);
  box-shadow: inset 0 -1.5px 0 var(--vr-color-text);
}

.count {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-2xs);
}

.pane {
  flex: 1;
  min-height: 0;
}

.pane.scroll {
  padding: var(--vr-space-5);
  overflow: auto;
}

.pipeline-body {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  max-width: 860px;
  margin: 0 auto;
}

.empty {
  margin: 0;
  padding: 40px 0;
  color: var(--vr-color-text-muted);
  text-align: center;
}

.pipeline-head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
}

.open-link {
  align-self: center;
  margin-left: auto;
  color: var(--vr-color-link);
  text-decoration: none;
}

.open-link:hover {
  text-decoration: underline;
}
</style>
