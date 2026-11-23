<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from 'vue';
import { Check, ChevronRight, Ellipsis, Eye, GitPullRequest, ListTodo, MessageSquare, Plus, RefreshCw, Search, SquareTerminal, TriangleAlert, X } from '../../../shared/ui/icons';
import { sidebarSearchRequests } from '../../../shared/model/sidebar';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { VContextMenu, useIsPhone } from '../../../shared/ui/kit';
import ArchivedExternalRow from './ArchivedExternalRow.vue';
import ArchivedRow from './ArchivedRow.vue';
import EndedRow from './EndedRow.vue';
import ExternalSessionRow from './ExternalSessionRow.vue';
import MarkdownPicker from './MarkdownPicker.vue';
import SessionRow from './SessionRow.vue';
import { useExternalSessions, NavNotices, openChannelFor, StatusIcon, useOpenWorkspace, useSessionDock } from '../../../entities/session';
import { ProjectActions, WorkspaceActions, RepoActions } from '../../../features/manage-workspace';
import { channelKeyOf, TypeIcon } from '../../../entities/jira-issue';
import { isDesktop } from '../../../bridge';
import { CleanupBadge, cleanupCount, openCleanup, removing as cleanupRemoving } from '../../../features/cleanup';
import { openUrl, type InboxItem } from '../../../shared/api/connectors/api';
import { ArchitectureAction, openArchitecture, useArchitecture } from '../../../features/architecture';
import { MrSessionNotice, OpenInSessionAction, useOpenInSession } from '../../../features/open-in-session';
import { numberLabel, pipelineLabel, pipelineTone, itemId, WorktreeBadge, openReview } from '../../../entities/merge-request';
import { t } from '../../../shared/lib/i18n/i18n';
import ExplorerTree from '../../../shared/ui/explorer-tree/ExplorerTree.vue';
import { useDock } from '../../../shared/model/dock';
import { InboxSection, OpenChannelAction } from '../../../features/attention';
import { ChannelsSection } from '../../../features/channels';
import { startCreate } from '../../../features/new-session';
import { openLeaf, type ReviewMeta, type ReviewVerdict, type WorkNode, treeLiveOnly, treeProjectsCollapsed, treeQuery, treeTab, useWorkTree } from '../../../features/work-tree';

import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
const props = withDefaults(defineProps<{ tabsShown?: readonly string[]; withInbox?: boolean }>(), { tabsShown: undefined, withInbox: true });
const phone = useIsPhone();

const dock = useDock();
const sessions = useSessionDock();
const { berth, nodes, tabs, activeTab, errors } = useWorkTree();
const tabIcons = { sessions: SquareTerminal, mrs: GitPullRequest, reviews: Eye, tasks: ListTodo, all: SquareTerminal };
const shownTabs = computed(() => (props.tabsShown ? tabs.value.filter((tab) => props.tabsShown?.includes(tab.id)) : tabs.value));

const filtering = computed(() => treeQuery.value.trim().length > 0);
const projectsOpen = computed(() => !treeProjectsCollapsed.value || filtering.value);
const liveShown = computed(() => activeTab.value === 'sessions' && treeLiveOnly.value);
const external = useExternalSessions();
const selected = computed(() => {
  const key = dock.activeKey.value;
  return key ? (external?.terminalOf.value.get(key) ?? key) : null;
});

const archMenu = useTemplateRef<InstanceType<typeof VContextMenu>>('archMenu');
const archItem = shallowRef<{ item: InboxItem; review?: ReviewMeta } | null>(null);
const { availability: archState } = useArchitecture(() => archItem.value?.item);
const mrSession = useOpenInSession();
const archItems = computed(() => {
  const target = archItem.value;
  if (!target) return [];
  const { item, review } = target;
  const state = archState.value;
  const items = [{ key: 'open-session', label: t('forge.openSession.action'), title: t('forge.openSession.title'), disabled: !berth || (mrSession.runs.value.get(itemId(item))?.busy ?? false), command: () => void mrSession.open(item, review?.refs) }, { key: 'open-browser', label: t('common.openInBrowser'), title: item.url, disabled: false, command: () => void openUrl(item.url) }, { key: 'open-channel', label: t('attention.channel.open'), title: t('attention.channel.openTitle', { key: channelKeyOf(item.source_branch, item.title) }), disabled: !berth, command: () => void openChannelFor(channelKeyOf(item.source_branch, item.title)) }];
  if (state.status === 'hidden') return items;
  const reason = state.status === 'disabled' ? state.reason : state.status === 'checking' ? t('forge.stratum.checking') : t('forge.stratum.title');
  return [...items, { key: 'architecture', label: t('forge.stratum.action'), title: reason, disabled: state.status !== 'ready', command: () => state.status === 'ready' && openArchitecture(state.params, numberLabel(item)) }];
});

function showArchMenu(event: MouseEvent, item: InboxItem, review?: ReviewMeta) {
  archItem.value = { item, review };
  event.preventDefault();
  archMenu.value?.show(event);
}

const jiraRefs = (review: ReviewMeta | undefined) => review?.refs.filter((ref) => !ref.startsWith('!')).join(', ') ?? '';
const verdictText = (verdict: ReviewVerdict) => (verdict === 'mine' ? t('workbench.review.approvedByMe') : verdict === 'approved' ? t('workbench.review.approved') : t('workbench.review.pending'));
const verdictTitle = (verdict: ReviewVerdict, note: string | undefined) => {
  const base = verdict === 'mine' ? t('workbench.review.approvedByMeTitle') : verdict === 'approved' ? t('workbench.review.approvedTitle') : t('workbench.review.pendingTitle');
  return note ? `${base}\n${note}` : base;
};
const shownTitle =(item: InboxItem) => (item.draft ? item.title.replace(/^(draft:|\[draft\]|\(draft\)|wip:)\s*/i, '') : item.title);

const projectAdd = useTemplateRef<InstanceType<typeof ProjectActions>>('projectAdd');
const openWorkspace = useOpenWorkspace();
const addWorkspaceId = computed(() => openWorkspace.value ?? berth?.tree.value[0]?.id);

const filterTitle = computed(() => {
  const shortcut = shortcutLabel('sidebar.search');
  return shortcut ? `${t('workbench.filter')} (${shortcut})` : t('workbench.filter');
});
const filterInput = useTemplateRef<InstanceType<typeof UiInput>>('filterInput');

function clearFilter(event: KeyboardEvent) {
  treeQuery.value = '';
  if (event.target instanceof HTMLElement) event.target.blur();
}

function clearFilterButton() {
  treeQuery.value = '';
  filterInput.value?.focus();
}

watch(sidebarSearchRequests, async () => {
  await nextTick();
  filterInput.value?.focus();
});

const moreMenu = useTemplateRef<InstanceType<typeof VContextMenu>>('moreMenu');
const moreItems = computed(() => {
  const items: { key: string; label: string; title?: string; checked?: boolean; command: () => void }[] = [];
  if (activeTab.value === 'sessions') items.push({ key: 'live-only', label: t('workbench.liveOnly'), checked: treeLiveOnly.value, command: () => (treeLiveOnly.value = !treeLiveOnly.value) });
  if (berth && isDesktop) {
    const title = cleanupRemoving.value ? t('cleanup.removing', { name: cleanupRemoving.value.name }) : cleanupCount.value ? t('cleanup.buttonCount', { count: cleanupCount.value }) : t('cleanup.button');
    items.push({ key: 'cleanup', label: title, command: openCleanup });
  }
  if (phone.value) {
    items.push({ key: 'quick-open', label: t('agents.layouts.quickOpen'), title: t('agents.layouts.quickOpenTitle'), command: () => (dock.switching.value = true) });
    if (berth) items.push({ key: 'metrics', label: t('agents.metrics.title'), title: t('agents.metrics.open'), command: () => sessions.openMetrics() });
  }
  return items;
});
</script>

<template>
  <section class="project-tree" data-testid="project-tree">
    <div class="head">
      <div class="tabs-box">
        <div v-if="shownTabs.length > 1" class="tabs" role="tablist" :aria-label="t('workbench.tabs')" :data-n="shownTabs.length">
          <UiButton
            v-for="tab in shownTabs"
            :key="tab.id"
            class="tab"
            role="tab"
            :aria-selected="activeTab === tab.id"
            :aria-label="tab.label"
            :title="tab.count ? `${tab.label} · ${tab.title}` : tab.label"
            :data-testid="`tree-tab-${tab.id}`"
            @click="treeTab = tab.id"
          >
            <component :is="tabIcons[tab.id]" :size="13" class="tab-icon" />
            <span class="tab-label">{{ tab.label }}</span>
            <span v-if="tab.count" class="badge" :data-tone="tab.tone" :data-testid="`tree-tab-count-${tab.id}`">{{ tab.count }}</span>
          </UiButton>
        </div>
      </div>
    </div>
    <label class="filter">
      <Search :size="14" />
      <UiInput ref="filterInput" v-model="treeQuery" :placeholder="t('workbench.filter')" :aria-label="t('workbench.filter')" :title="filterTitle" data-testid="tree-filter" @keydown.esc="clearFilter" />
      <UiButton v-if="treeQuery" type="button" class="icon" :title="t('workbench.clearFilter')" :aria-label="t('workbench.clearFilter')" data-testid="tree-filter-clear" @click="clearFilterButton"><X :size="14" /></UiButton>
    </label>
    <InboxSection v-if="withInbox && activeTab === 'sessions'" class="pinned-inbox" :query="treeQuery" />
    <div class="scroll">
      <NavNotices />
      <MrSessionNotice v-for="run in mrSession.runs.value.values()" :key="run.id" :run="run">
        <template #project="{ repo }"><RepoActions :repo="repo" /></template>
      </MrSessionNotice>
      <p v-for="error in errors" :key="`${error.connector}:${error.message}`" class="strip" role="alert" data-testid="tree-error" :title="error.message">{{ error.connector }}: {{ error.message }}</p>
      <ChannelsSection v-if="activeTab === 'sessions' && !filtering" />
      <div v-if="activeTab === 'sessions'" class="section-head" data-testid="project-tree-head">
        <UiButton class="section-toggle" :aria-expanded="projectsOpen" :disabled="filtering" data-testid="project-tree-toggle" @click="treeProjectsCollapsed = !treeProjectsCollapsed">
          <ChevronRight :size="14" class="chevron vr-chevron" />
          <span class="section-title">{{ t('workbench.tree') }}</span>
        </UiButton>
        <span class="section-actions">
          <UiButton v-if="moreItems.length" class="icon more" :data-active="liveShown" :title="t('workbench.more')" :aria-label="t('workbench.more')" aria-haspopup="menu" data-testid="tree-more" @click="moreMenu?.show($event)"><Ellipsis :size="14" /></UiButton>
          <UiButton v-if="addWorkspaceId" class="icon" :title="t('agents.project.add')" :aria-label="t('agents.project.add')" data-testid="project-add" @click="projectAdd?.openAdd()"><Plus :size="14" /></UiButton>
        </span>
      </div>
      <ProjectActions v-if="activeTab === 'sessions' && addWorkspaceId" ref="projectAdd" :workspace-id="addWorkspaceId" bare @added="treeProjectsCollapsed = false" />
      <ExplorerTree
        v-show="activeTab !== 'sessions' || projectsOpen"
        :nodes="nodes"
        :storage-key="`berth.tree.expanded.${activeTab}`"
        :label="t('workbench.tree')"
        :selected="selected"
        :reveal-all="filtering || liveShown"
        test-id="explorer-tree"
        @open="openLeaf"
      >
        <template #lead="{ node, icon }">
          <StatusIcon v-if="node.kind === 'project' && node.data?.rollup?.total" :status="node.data.rollup.status" :unseen="node.data.rollup.unseen" />
          <component :is="icon" v-else-if="icon" :size="14" class="kind-icon" />
        </template>
        <template #rollup="{ node }">
          <StatusIcon v-if="node.kind !== 'project' && node.data?.rollup?.total" :status="node.data.rollup.status" :unseen="node.data.rollup.unseen" />
          <span v-if="node.data?.pipeline && node.data.pipeline !== 'none'" class="pipe" :title="pipelineLabel(node.data.pipeline)"><StatusIcon :tone="pipelineTone(node.data.pipeline)" /></span>
        </template>
        <template #leaf="{ node, data }">
          <span v-if="data?.item" class="line" data-testid="tree-mr-row" :data-mr="`${data.item.project}!${data.item.number}`" @contextmenu="showArchMenu($event, data.item, data.review)">
            <span class="key">{{ numberLabel(data.item) }}</span>
            <span v-if="jiraRefs(data.review)" class="refs" data-testid="tree-mr-refs">{{ jiraRefs(data.review) }}</span>
            <span class="title"><span v-if="data.item.draft" class="draft">{{ t('forge.draft') }}</span>{{ shownTitle(data.item) }}</span>
            <span v-if="data.verdict" class="verdict" :data-verdict="data.verdict" :title="verdictTitle(data.verdict, data.review?.note)" data-testid="tree-mr-verdict">{{ verdictText(data.verdict) }}</span>
            <span v-else-if="data.review?.note" class="verdict note" :title="data.review.note" data-testid="tree-mr-note">{{ data.review.note }}</span>
            <span v-if="data.item.unresolved_threads" class="threads" :title="t('forge.unresolvedThreads', { count: data.item.unresolved_threads })"><MessageSquare :size="12" />{{ data.item.unresolved_threads }}</span>
            <span v-if="data.item.pipeline !== 'none'" class="pipe" :title="pipelineLabel(data.item.pipeline)"><StatusIcon :tone="pipelineTone(data.item.pipeline)" /></span>
          </span>
          <span v-else-if="data?.issue" class="line" data-testid="tree-issue-row">
            <TypeIcon :type="data.issue.issue_type" />
            <span class="key">{{ data.issue.key }}</span>
            <span class="title">{{ data.issue.summary }}</span>
          </span>
          <span v-else class="line">
            <span class="title">{{ node.label }}</span>
          </span>
        </template>
        <template #row="{ node }">
          <SessionRow
            v-if="node.kind === 'session' && node.data?.session"
            :session="node.data.session"
            :linked="node.data.linked"
            :active="node.id === selected"
            @open="(split) => node.data?.session && sessions.openSession(node.data.session, undefined, false, split)"
            @open-conversation="node.data.linked && sessions.openLinkedConversation(node.data.session, node.data.linked)"
          />
          <ExternalSessionRow v-else-if="node.kind === 'external' && node.data?.external" :session="node.data.external" :active="node.id === dock.activeKey.value" @open="(split) => node.data?.external && sessions.openExternal(node.data.external, split)" />
          <EndedRow v-else-if="node.kind === 'ended-session' && node.data?.session" :session="node.data.session" :where="node.data.where" />
          <ArchivedRow v-else-if="node.kind === 'archived' && node.data?.session" :session="node.data.session" :where="node.data.where" />
          <ArchivedExternalRow v-else-if="node.kind === 'archived-external' && node.data?.external" :session="node.data.external" @open="(split) => node.data?.external && sessions.openExternal(node.data.external, split)" />
          <div v-else-if="node.kind === 'review-state' && node.data?.reviewState" class="review-state" :data-error="!!node.data.reviewState.error" :title="node.data.reviewState.error ?? undefined" role="status" data-testid="review-state">
            <template v-if="node.data.reviewState.error"><TriangleAlert :size="12" /><span class="review-text">{{ node.data.reviewState.error }}</span></template>
            <template v-else><RefreshCw :size="12" class="spin" /><span class="review-text">{{ node.label }}</span></template>
          </div>
          <ul v-else-if="node.kind === 'review-notes' && node.data?.notes" class="review-notes" :aria-label="node.label" data-testid="review-notes">
            <li v-for="(note, index) in node.data.notes" :key="index" :title="note.text"><b v-if="note.refs.length">{{ note.refs.join(', ') }}</b> {{ note.text }}</li>
          </ul>
        </template>
        <template #badges="{ node }">
          <CleanupBadge v-if="node.kind === 'worktree' && node.data?.worktree && !node.data.worktree.main" :worktree-id="node.data.worktree.id" />
          <WorktreeBadge v-if="node.kind === 'worktree' && node.data?.worktree" :path="node.data.worktree.path" :branch="node.data.worktree.branch" :summary="node.data.worktreeMr" @open="(ref) => openReview(ref)">
            <template #pipeline="{ pipeline }"><StatusIcon v-if="pipeline !== 'none'" :tone="pipelineTone(pipeline)" /></template>
          </WorktreeBadge>
        </template>
        <template #actions="{ node }">
          <template v-if="node.kind === 'worktree' && node.data?.worktree">
            <OpenChannelAction v-if="!node.data.worktree.main" :channel-key="channelKeyOf(node.data.worktree.branch)" variant="icon" />
            <MarkdownPicker :root="node.data.worktree.path" @open="sessions.openFile($event, { worktreeId: node.data.worktree.id })" />
            <UiButton class="icon" :title="t('agents.newSession.action')" :aria-label="t('agents.newSession.action')" data-testid="worktree-new-session" @click="startCreate(node.data.worktree.id, true)"><Plus :size="14" /></UiButton>
          </template>
          <WorkspaceActions v-else-if="node.kind === 'workspace' && node.data?.workspaceId" :workspace-id="node.data.workspaceId" />
          <RepoActions v-else-if="node.kind === 'repo' && node.data?.repo" :repo="node.data.repo" />
          <template v-else-if="node.data?.item">
            <OpenInSessionAction :item="node.data.item" :refs="node.data.review?.refs" variant="icon" />
            <ArchitectureAction :item="node.data.item" variant="icon" />
          </template>
          <ProjectActions
            v-else-if="node.kind === 'project' && node.data?.projectId"
            :project-id="node.data.projectId"
            @resumed="sessions.openWhenListed({ Agent: $event })"
            @create="startCreate($event, true)"
          />
        </template>
        <template #empty>
          <p class="empty" data-testid="tree-empty">{{ filtering ? t('workbench.nothingFound') : t('workbench.empty') }}</p>
        </template>
      </ExplorerTree>
    </div>
    <VContextMenu ref="moreMenu" :model="moreItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :title="item.title" :data-testid="`tree-more-${item.key}`">
          <span class="more-label">{{ item.label }}</span>
          <Check v-if="item.checked" :size="14" />
        </a>
      </template>
    </VContextMenu>
    <VContextMenu ref="archMenu" :model="archItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :title="item.title" :data-testid="`mr-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
  </section>
</template>

<style scoped>
.project-tree {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  font-size: var(--vr-font-size-label);
}

.filter {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  margin: 0 var(--vr-space-1) var(--vr-space-1);
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-nav-muted);
  background: var(--vr-color-nav-hover);
  border-radius: var(--vr-radius-md);
}

.filter input {
  flex: 1;
  min-width: 0;
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-1) var(--vr-space-1) var(--vr-space-1);
}

.tabs-box {
  flex: 1;
  min-width: 0;
  container: tabs / inline-size;
}

.tabs {
  display: flex;
  gap: 2px;
  padding: 2px;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.tab {
  display: inline-flex;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  gap: 4px;
  min-width: 0;
  height: 24px;
  padding: 0 4px;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  transition: background var(--vr-motion-overlay-in, 0.12s) ease;
}

.tab:hover {
  color: var(--vr-color-text);
}

.tab[aria-selected='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: var(--vr-elevation-card);
}

.tab-icon {
  display: none;
  flex: none;
}

.tab-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

.badge[data-tone='accent'] {
  color: var(--vr-color-text);
}

.badge[data-tone='alert'] {
  color: var(--vr-status-feedback);
}

@container tabs (max-width: 196px) {
  .tab-label {
    display: none;
  }

  .tab-icon {
    display: block;
  }
}

@container tabs (max-width: 248px) {
  .tabs[data-n='4'] .tab-label {
    display: none;
  }

  .tabs[data-n='4'] .tab-icon {
    display: block;
  }
}

.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.icon[aria-pressed='true'],
.icon.more[data-active='true'] {
  color: var(--vr-color-accent);
  background: var(--vr-color-nav-active);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.pinned-inbox {
  flex: none;
  margin-inline: var(--vr-space-1);
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 0 var(--vr-space-1) var(--vr-space-2);
}

.strip {
  margin: 0 0 var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2);
  overflow: hidden;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.kind-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.scroll :deep(.label[data-kind='worktree']) {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.scroll :deep(.label[data-kind='sessions']),
.scroll :deep(.label[data-kind='mrs']),
.scroll :deep(.label[data-kind='tasks']),
.scroll :deep(.label[data-kind='section']),
.scroll :deep(.label[data-kind='archive']),
.scroll :deep(.label[data-kind='ended']),
.scroll :deep(.label[data-kind='folders']) {
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.scroll :deep(.node-actions:has(.inline)),
.extra :deep(.actions:has(.inline)) {
  flex: 1 0 100%;
  padding: 0 var(--vr-space-2) var(--vr-space-2) calc(var(--depth) * var(--indent) + 24px);
  opacity: 1;
}

.scroll :deep(.custom .row) {
  padding-left: var(--vr-space-2);
}

.scroll :deep(.custom .archived .line),
.scroll :deep(.custom .ended .line) {
  padding-left: var(--vr-space-2);
}

.extra {
  padding-left: 12px;
}

.extra :deep(.actions) {
  justify-content: flex-start;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-1);
  height: 28px;
  padding-left: 0;
}

.section-toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 7px;
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 6px;
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.section-toggle:hover:enabled {
  background: var(--vr-color-nav-hover);
}

.section-toggle:disabled {
  cursor: default;
}

.section-toggle .chevron {
  flex: none;
  width: 10px;
  height: 10px;
  color: var(--vr-color-text-subtle);
}

.section-title {
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.section-actions {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 2px;
}

.pipe {
  display: inline-flex;
  flex: none;
}

.pipe :deep(.status-icon) {
  width: 12px;
  height: 12px;
}

.line {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
}

.key {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.draft {
  margin-right: var(--vr-space-1);
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.threads {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 2px;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-conflict);
}

.refs {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-accent);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.verdict {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.verdict.note {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.verdict[data-verdict='approved'],
.verdict[data-verdict='mine'] {
  color: var(--vr-color-success, var(--vr-color-accent));
}

.review-state {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-2) + 14px);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.review-state[data-error='true'] {
  color: var(--vr-color-danger);
}

.review-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.review-notes {
  margin: 0;
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-2) + 14px);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  list-style: none;
}

.review-notes li {
  display: -webkit-box;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
