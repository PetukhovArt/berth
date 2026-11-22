import { computed, effectScope, shallowRef, watch } from 'vue';
import type { DockviewApi } from 'dockview-vue';
import { splitPosition, titleKey, useDock, type PanelPosition, type PanelSpec } from '../../../shared/model/dock';
import { FILE_PANEL_PREFIX, fileNameOf, filePanelId, isFilePanel, normalizePath } from '../../../shared/lib/markdownFiles';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSettings } from '../../../shared/config/settings/store';
import { externalKey, externalTarget, isExternalPanel, useExternalSessions, type ExternalSession } from './external';
import { markSeen, onAgentSpawned, sessionKey, useBerth, type SessionNode, type SessionRef, type WorktreeNode } from './berth';
import { loadOpenTabs, saveOpenTabs } from './openTabs';
import { sessionPaths } from '../lib/sessionPaths';

export const METRICS_PANEL = 'metrics';

const isSessionPanel = (id: string) => id.startsWith('agent:') || id.startsWith('terminal:');

function filePanel(path: string, hash = ''): PanelSpec {
  return { id: filePanelId(path), component: 'markdown', title: fileNameOf(path), params: { path: normalizePath(path), hash }, renderer: 'always' };
}

const isLooking = () => document.visibilityState === 'visible' && document.hasFocus();

const reviveFile =(id: string) => (isFilePanel(id) ? filePanel(id.slice(FILE_PANEL_PREFIX.length)) : undefined);

function createSessionDock() {
  const berth = useBerth();
  const dock = useDock();
  const { activeKey, visible, withDock, openPanel } = dock;
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

  const looking = shallowRef(isLooking());
  const watchLooking = () => (looking.value = isLooking());
  for (const event of ['focus', 'blur']) window.addEventListener(event, watchLooking);
  document.addEventListener('visibilitychange', watchLooking);

  const shownUnseenAgents = computed(() => {
    if (!visible.value || !looking.value || berth?.connection.value.state !== 'connected') return [];
    const terminalOf = useExternalSessions()?.terminalOf.value;
    const agents: string[] = [];
    for (const id of dock.shown.value) {
      const session = berth.sessions.value.get(terminalOf?.get(id) ?? id);
      if (session?.unseen && 'Agent' in session.ref) agents.push(session.ref.Agent);
    }
    return agents;
  });

  watch(shownUnseenAgents, (agents, previous) => {
    for (const agent of agents) if (!previous?.includes(agent)) void markSeen(agent);
  });

  watch(
    () => berth?.sessions.value,
    (sessions) => {
      if (!sessions || berth?.connection.value.state !== 'connected') return;
      const api = dock.api.value;
      if (api) reopen(api);
      if (api) closeGone(api);
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

  function openSpawned(agent: string, spawnedBy: string) {
    const api = dock.api.value;
    const session = berth?.sessions.value.get(sessionKey({ Agent: agent }));
    if (!api || !session || api.getPanel(session.key)) return;
    const parent = sessionKey({ Agent: spawnedBy });
    const terminalOf = useExternalSessions()?.terminalOf.value;
    const anchor = api.panels.find((panel) => (terminalOf?.get(panel.id) ?? panel.id) === parent);
    if (!anchor) return;
    api.addPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always', position: { referencePanel: anchor.id, direction: 'within' }, inactive: true });
  }

  onAgentSpawned(openSpawned);

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
      api.addPanel({ ...filePanel(path, hash), ...(position ? { position } : {}) });
    });
  }

  function openExternal(session: ExternalSession, split = false, beside?: string) {
    const anchor: PanelPosition | boolean = beside && dock.api.value?.getPanel(beside) ? { referencePanel: beside, direction: 'within' } : split;
    openPanel({ id: externalKey(session), component: 'conversation', title: session.title, params: { harness: session.harness, id: session.id } }, anchor);
  }

  function openKey(key: string) {
    const node = berth?.sessions.value.get(key);
    if (node) {
      openSession(node);
      return;
    }
    const session = useExternalSessions()?.anyByKey.value.get(key);
    if (session) openExternal(session);
  }

  function openMetrics() {
    openPanel({ id: METRICS_PANEL, component: 'metrics', title: t('agents.metrics.title'), params: titleKey('agents.metrics.title') });
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
    if (!reopening) return;
    const { keys, active } = reopening;
    const restore = useSettings().values.value['sessions.restoreOnRestart'];
    const waiting = keys.filter(isSessionPanel);
    if (restore)
      for (const key of keys) {
        const panel = isSessionPanel(key) || api.getPanel(key) ? undefined : dock.revive(key);
        if (panel) api.addPanel(panel);
      }
    reopening = waiting.length ? { keys: waiting, active } : null;
    const sessions = berth?.sessions.value;
    if (reopening && (!sessions?.size || berth?.connection.value.state !== 'connected')) return;
    reopening = null;
    if (!restore) return;
    for (const key of waiting) {
      const session = sessions?.get(key);
      if (session && !api.getPanel(key)) addSessionPanel(session, api);
    }
    if (active) api.getPanel(active)?.api.setActive();
  }

  function closeGone(api: DockviewApi) {
    const sessions = berth?.sessions.value;
    if (!sessions || berth?.connection.value.state !== 'connected') return;
    for (const panel of api.panels) if (isSessionPanel(panel.id) && !sessions.has(panel.id)) panel.api.close();
  }

  function remember(api: DockviewApi) {
    closeGone(api);
    if (reopening) return;
    const keep = useSettings().values.value['sessions.restoreOnRestart'];
    const keys = api.panels.map((panel) => panel.id).filter((id) => isSessionPanel(id) || dock.revive(id) !== undefined);
    saveOpenTabs(keep ? { keys, active: api.activePanel?.id ?? null } : null);
  }

  dock.revivable(reviveFile);
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
    openKey,
    openWhenListed,
    openLinkedConversation,
    openExternal,
    openFile,
    openMetrics,
  };
}

let store: ReturnType<typeof createSessionDock> | null = null;

export function useSessionDock() {
  store ??= effectScope(true).run(createSessionDock) as ReturnType<typeof createSessionDock>;
  return store;
}
