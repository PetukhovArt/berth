import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, watch } from 'vue';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-vue';
import { externalKey, externalTarget, isExternalPanel, useExternalSessions, type ExternalSession } from '../../entities/session/model/external';
import { markSeen, sessionKey, useBerth, worktreesOfProject, type SessionNode, type SessionRef, type WorktreeNode } from '../../entities/session/model/berth';
import { FILE_PANEL_PREFIX, fileNameOf, filePanelId, isFilePanel, normalizePath } from '../lib/markdownFiles';
import { sessionPaths } from '../../entities/session/lib/sessionPaths';
import type { ChangeRequestRef } from '../api/connectors/api';
import { numberLabel } from '../../entities/merge-request/lib/format';
import { itemId } from '../../entities/merge-request/model/inbox';
import { architecturePanelId, type ArchitectureParams } from '../../features/architecture/model/stratum';
import { t } from '../lib/i18n/i18n';
import { useIsPhone } from '@ui/kit';
import { navigate, view } from './nav';
import { useSettings } from '../config/settings/store';
import { loadOpenTabs, saveOpenTabs } from '../../entities/session/model/openTabs';
import type { WorkNode } from '../../features/work-tree/model/tree';

export const METRICS_PANEL = 'metrics';

type PanelSpec = { id: string; component: string; title: string; params?: Record<string, unknown>; renderer?: 'always' };
type PanelPosition = { referencePanel: string; direction: 'within' | 'right' | 'below' | 'left' };
type Carried = { id: string; component: string; title: string; params: Record<string, unknown> };

export const issuePanelId = (connector: string, key: string) => `jira:${connector}/${key}`;
const isSessionPanel = (id: string) => id.startsWith('agent:') || id.startsWith('terminal:');

function createDock() {
  const berth = useBerth();
  const mobile = useIsPhone();
  const dock = shallowRef<DockviewApi | null>(null);
  const activeKey = shallowRef<string | null>(null);
  const activeTitle = shallowRef('');
  const visible = shallowRef(false);
  const creatingIn = shallowRef<{ worktree: string | null } | null>(null);
  const openingKey = shallowRef<string | null>(null);
  let openingBeside: string | undefined;
  let openingReplace = false;
  const detail = shallowRef(false);
  const switching = shallowRef(false);
  const pending: ((api: DockviewApi) => void)[] = [];
  let carried: { panels: Carried[]; active: string | null } = { panels: [], active: null };
  let reopening = loadOpenTabs();

  const paths = computed(() => sessionPaths(berth?.tree.value ?? []));
  const activePath = computed(() => (activeKey.value ? paths.value.get(activeKey.value) : undefined));
  const activeSession = computed(() => (activeKey.value ? berth?.sessions.value.get(activeKey.value) : undefined));

  watch(
    () => [...(berth?.sessions.value.values() ?? [])].map((session) => [session.key, session.name] as const),
    (names) => {
      const api = dock.value;
      if (!api) return;
      for (const [key, name] of names) {
        const panel = api.getPanel(key);
        if (!panel || panel.title === name) continue;
        panel.api.setTitle(name);
        if (key === activeKey.value) activeTitle.value = name;
      }
    },
  );

  function findWorktree(id: string | null | undefined): WorktreeNode | null {
    if (!id) return null;
    for (const workspace of berth?.tree.value ?? []) for (const project of workspace.projects) for (const worktree of project.worktrees) if (worktree.id === id) return worktree;
    return null;
  }

  function worktreeOfFile(file: string): WorktreeNode | null {
    let best: WorktreeNode | null = null;
    for (const workspace of berth?.tree.value ?? [])
      for (const project of workspace.projects)
        for (const worktree of project.worktrees) {
          const root = normalizePath(worktree.path).replace(/\/+$/, '');
          if (file.startsWith(`${root}/`) && (!best || root.length > best.path.length)) best = worktree;
        }
    return best;
  }

  const activeWorktree = computed(() => {
    const byPath = findWorktree(activePath.value?.worktree.id);
    const key = activeKey.value;
    return byPath || !key || !isFilePanel(key) ? byPath : worktreeOfFile(key.slice(FILE_PANEL_PREFIX.length));
  });

  const activeWorktreeId = computed(() => {
    const known = activeWorktree.value?.id;
    const key = activeKey.value;
    if (known || !key || !isExternalPanel(key)) return known ?? null;
    const session = useExternalSessions()?.anyByKey.value.get(key);
    return session ? (externalTarget(session, berth?.entities.value)?.worktreeId ?? null) : null;
  });

  const activeUnseenAgent = computed(() => {
    const session = activeSession.value;
    return visible.value && session?.unseen && 'Agent' in session.ref ? session.ref.Agent : null;
  });

  watch(activeUnseenAgent, (agent) => {
    if (agent) void markSeen(agent);
  });

  watch(
    () => berth?.sessions.value,
    (sessions) => {
      if (!sessions || berth?.connection.value.state !== 'connected') return;
      const api = dock.value;
      if (api) reopen(api);
      if (api) for (const panel of api.panels) if (isSessionPanel(panel.id) && !sessions.has(panel.id)) panel.api.close();
      const opening = openingKey.value ? sessions.get(openingKey.value) : undefined;
      if (!opening) return;
      openingKey.value = null;
      openSession(opening, openingBeside, openingReplace);
      openingBeside = undefined;
      openingReplace = false;
    },
  );

  function flush() {
    const api = dock.value;
    if (!api || !visible.value) return;
    for (const run of pending.splice(0)) run(api);
  }

  function withDock(run: (api: DockviewApi) => void, reveal = true) {
    if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
    if (reveal && mobile.value) detail.value = true;
    pending.push(run);
    flush();
  }

  function splitPosition(api: DockviewApi): PanelPosition | undefined {
    return api.activePanel ? { referencePanel: api.activePanel.id, direction: 'right' } : undefined;
  }

  function openPanel(panel: PanelSpec, split = false) {
    withDock((api) => {
      const existing = api.getPanel(panel.id);
      const position = split ? splitPosition(api) : undefined;
      if (existing) existing.api.setActive();
      else api.addPanel(position ? { ...panel, position } : panel);
    });
  }

  function toggleMaximize(panelId?: string) {
    const api = dock.value;
    if (!api) return;
    if (api.hasMaximizedGroup()) {
      api.exitMaximizedGroup();
      return;
    }
    const panel = panelId ? api.getPanel(panelId) : api.activePanel;
    if (panel && api.groups.length > 1) panel.api.maximize();
  }

  function sessionPosition(session: SessionNode, api: DockviewApi, split: boolean): PanelPosition | undefined {
    if (split) return splitPosition(api);
    const worktree = paths.value.get(session.key)?.worktree.id;
    const sibling = api.panels.find((panel) => panel.id !== session.key && paths.value.get(panel.id)?.worktree.id === worktree);
    return sibling ? { referencePanel: sibling.id, direction: 'within' } : undefined;
  }

  function addSessionPanel(session: SessionNode, api: DockviewApi, split = false) {
    const position = sessionPosition(session, api, split);
    api.addPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always', ...(position ? { position } : {}) });
  }

  function openSession(session: SessionNode, beside?: string, replace = false, split = false) {
    withDock((api) => {
      const existing = api.getPanel(session.key);
      const anchor = beside ? api.getPanel(beside) : undefined;
      if (existing) existing.api.setActive();
      else if (anchor) api.addPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always', position: { referencePanel: anchor.id, direction: 'within' } });
      else addSessionPanel(session, api, split);
      if (replace) anchor?.api.close();
    });
  }

  function openLinkedConversation(session: SessionNode, linked: ExternalSession) {
    withDock((api) => {
      const id = externalKey(linked);
      const existing = api.getPanel(id);
      if (existing) {
        existing.api.setActive();
        return;
      }
      if (!api.getPanel(session.key)) addSessionPanel(session, api);
      api.addPanel({ id, component: 'conversation', title: session.name, params: { harness: linked.harness, id: linked.id }, position: { referencePanel: session.key, direction: 'within' } });
    });
  }

  function openFile(path: string, options: { hash?: string; worktreeId?: string; from?: string } = {}) {
    withDock((api) => {
      const id = filePanelId(path);
      const hash = options.hash ?? '';
      const existing = api.getPanel(id);
      if (existing) {
        existing.api.updateParameters({ path: normalizePath(path), hash });
        existing.api.setActive();
        return;
      }
      const anchor =
        options.from && api.getPanel(options.from)
          ? options.from
          : options.worktreeId
            ? api.panels.find((panel) => paths.value.get(panel.id)?.worktree.id === options.worktreeId)?.id
            : undefined;
      const position: PanelPosition | undefined = anchor ? { referencePanel: anchor, direction: 'within' } : undefined;
      api.addPanel({ id, component: 'markdown', title: fileNameOf(path), params: { path: normalizePath(path), hash }, renderer: 'always', ...(position ? { position } : {}) });
    });
  }

  function openReview(ref: ChangeRequestRef, title?: string, split = false) {
    openPanel({ id: itemId(ref), component: 'review', title: title ?? `${ref.project.split('/').pop() ?? ref.project} ${ref.number}`, renderer: 'always', params: { ...ref } }, split);
  }

  function openArchitecture(params: ArchitectureParams, label: string) {
    openPanel({ id: architecturePanelId(params), component: 'architecture', title: t('forge.stratum.tab', { label }), renderer: 'always', params: { ...params } });
  }

  function openIssue(connector: string, key: string, split = false) {
    openPanel({ id: issuePanelId(connector, key), component: 'issue', title: key, params: { connector, key } }, split);
  }

  function openExternal(session: ExternalSession, split = false) {
    openPanel({ id: externalKey(session), component: 'conversation', title: session.title, params: { harness: session.harness, id: session.id } }, split);
  }

  function openLeaf(node: WorkNode, split = false) {
    const data = node.data;
    if (node.kind === 'session' && data?.session) openSession(data.session, undefined, false, split);
    else if (data?.external) openExternal(data.external, split);
    else if (data?.item) openReview(data.item, `${numberLabel(data.item)} ${data.item.title}`, split);
    else if (data?.issue) openIssue(data.issue.connector, data.issue.key, split);
  }

  function openMetrics() {
    openPanel({ id: METRICS_PANEL, component: 'metrics', title: t('agents.metrics.title') });
  }

  function openWhenListed(session: SessionRef, beside?: string, replace = false) {
    const key = sessionKey(session);
    const node = berth?.sessions.value.get(key);
    if (node) openSession(node, beside, replace);
    else {
      openingBeside = beside;
      openingReplace = replace;
      openingKey.value = key;
    }
  }

  function openMrSession(session: SessionRef, mr: ChangeRequestRef, title: string) {
    openReview(mr, title);
    openWhenListed(session, itemId(mr));
  }

  function startCreate(worktreeId: string | null) {
    if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
    creatingIn.value = { worktree: worktreeId };
  }

  function startCreateInMain() {
    const entities = berth?.entities.value;
    const active = activeWorktreeId.value;
    const project = (active ? entities?.worktrees.get(active)?.project_id : undefined) ?? berth?.tree.value.flatMap((workspace) => workspace.projects)[0]?.id;
    const target = entities && project ? (worktreesOfProject(entities, project).find((worktree) => worktree.is_main) ?? worktreesOfProject(entities, project)[0]) : undefined;
    startCreate(target?.id ?? null);
  }

  function onCreated(session: SessionRef) {
    creatingIn.value = null;
    openWhenListed(session);
  }

  function cancelCreate() {
    creatingIn.value = null;
  }

  function restore(api: DockviewApi) {
    const sessions = berth?.sessions.value;
    for (const panel of carried.panels) {
      if (isSessionPanel(panel.id) && !sessions?.has(panel.id)) continue;
      const always = panel.component === 'session' || panel.component === 'markdown' || panel.component === 'review' || panel.component === 'architecture';
      api.addPanel({ id: panel.id, component: panel.component, title: panel.title, params: panel.params, ...(always ? { renderer: 'always' as const } : {}) });
    }
    if (carried.active) api.getPanel(carried.active)?.api.setActive();
    carried = { panels: [], active: null };
  }

  function reopen(api: DockviewApi) {
    const sessions = berth?.sessions.value;
    if (!reopening || !sessions?.size || berth?.connection.value.state !== 'connected') return;
    const { keys, active } = reopening;
    reopening = null;
    if (!useSettings().values.value['sessions.restoreOnRestart']) return;
    for (const key of keys) {
      const session = sessions.get(key);
      if (session && !api.getPanel(key)) addSessionPanel(session, api);
    }
    if (active) api.getPanel(active)?.api.setActive();
  }

  function remember(api: DockviewApi) {
    if (reopening) return;
    const keep = useSettings().values.value['sessions.restoreOnRestart'];
    saveOpenTabs(keep ? { keys: api.panels.map((panel) => panel.id).filter(isSessionPanel), active: api.activePanel?.id ?? null } : null);
  }

  function useHost() {
    let listeners: { dispose(): void }[] = [];
    visible.value = true;

    function onReady({ api }: DockviewReadyEvent) {
      dock.value = api;
      listeners = [
        api.onDidActivePanelChange(({ panel }) => {
          activeKey.value = panel?.id ?? null;
          activeTitle.value = panel?.title ?? '';
          remember(api);
        }),
        api.onDidAddPanel(() => remember(api)),
        api.onDidRemovePanel(() => remember(api)),
      ];
      const queued = pending.splice(0);
      restore(api);
      reopen(api);
      pending.push(...queued);
      flush();
    }

    onActivated(() => {
      visible.value = true;
      flush();
    });
    onDeactivated(() => {
      visible.value = false;
    });
    onBeforeUnmount(() => {
      const api = dock.value;
      if (api)
        carried = {
          panels: api.panels.map((panel) => ({ id: panel.id, component: panel.view.contentComponent, title: panel.title ?? panel.id, params: { ...panel.params } })),
          active: activeKey.value,
        };
      for (const listener of listeners) listener.dispose();
      dock.value = null;
      visible.value = false;
    });

    return { onReady };
  }

  return {
    berth,
    activeKey,
    activeTitle,
    activeSession,
    activeWorktree,
    activeWorktreeId,
    worktreeOfFile,
    paths,
    creatingIn,
    detail,
    switching,
    openSession,
    openLinkedConversation,
    openFile,
    openReview,
    openArchitecture,
    openIssue,
    openExternal,
    openLeaf,
    openMetrics,
    toggleMaximize,
    openWhenListed,
    openMrSession,
    startCreate,
    startCreateInMain,
    onCreated,
    cancelCreate,
    useHost,
  };
}

let store: ReturnType<typeof createDock> | null = null;

export function useDock() {
  store ??= createDock();
  return store;
}

export function startNewSession(): void {
  const dock = useDock();
  const active = dock.activeWorktreeId.value;
  if (active) dock.startCreate(active);
  else dock.startCreateInMain();
}

export function toggleMaximizedGroup(): void {
  useDock().toggleMaximize();
}

export function toggleQuickOpen(): void {
  const { switching } = useDock();
  switching.value = !switching.value;
}
