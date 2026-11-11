import { computed, shallowRef, watch } from 'vue';
import type { DockviewApi } from 'dockview-vue';
import { splitPosition, useDock, type PanelPosition } from '../../../shared/model/dock';
import { FILE_PANEL_PREFIX, fileNameOf, filePanelId, isFilePanel, normalizePath } from '../../../shared/lib/markdownFiles';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSettings } from '../../../shared/config/settings/store';
import { externalKey, externalTarget, isExternalPanel, useExternalSessions, type ExternalSession } from './external';
import { markSeen, sessionKey, useBerth, type SessionNode, type SessionRef, type WorktreeNode } from './berth';
import { loadOpenTabs, saveOpenTabs } from './openTabs';
import { sessionPaths } from '../lib/sessionPaths';

export const METRICS_PANEL = 'metrics';

const isSessionPanel = (id: string) => id.startsWith('agent:') || id.startsWith('terminal:');

function createSessionDock() {
  const berth = useBerth();
  const dock = useDock();
  const { activeKey, activeTitle, visible, withDock, openPanel } = dock;
  const openingKey = shallowRef<string | null>(null);
  let openingBeside: string | undefined;
  let openingReplace = false;
  let reopening = loadOpenTabs();

  const paths = computed(() => sessionPaths(berth?.tree.value ?? []));
  const activePath = computed(() => (activeKey.value ? paths.value.get(activeKey.value) : undefined));
  const activeSession = computed(() => (activeKey.value ? berth?.sessions.value.get(activeKey.value) : undefined));

  watch(
    () => [...(berth?.sessions.value.values() ?? [])].map((session) => [session.key, session.name] as const),
    (names) => {
      const api = dock.api.value;
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
      const api = dock.api.value;
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

  function openExternal(session: ExternalSession, split = false) {
    openPanel({ id: externalKey(session), component: 'conversation', title: session.title, params: { harness: session.harness, id: session.id } }, split);
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

  dock.hook({
    keep: (id) => !isSessionPanel(id) || (berth?.sessions.value.has(id) ?? false),
    ready: reopen,
    change: remember,
  });

  return {
    paths,
    activeSession,
    activeWorktree,
    activeWorktreeId,
    worktreeOfFile,
    openSession,
    openWhenListed,
    openLinkedConversation,
    openExternal,
    openFile,
    openMetrics,
  };
}

let store: ReturnType<typeof createSessionDock> | null = null;

export function useSessionDock() {
  store ??= createSessionDock();
  return store;
}
