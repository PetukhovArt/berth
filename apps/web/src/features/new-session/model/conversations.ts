import { computed, shallowRef, watch, type ComputedRef, type Ref, type ShallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError, externalKey, isHarness, listProjectConversations, placeExternal, useBerth, useExternalSessions, type Entities, type Harness, type OrphanedSession } from '../../../entities/session';

export type ConversationChoice = {
  key: string;
  harness: Harness;
  id: string;
  title: string;
  checkout: string;
  at: number;
  busy: string | null;
};

export type ProjectConversations = {
  choices: ComputedRef<ConversationChoice[]>;
  loading: ShallowRef<boolean>;
  error: ShallowRef<string | null>;
};

const normalize = (path: string) => path.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase();

function checkoutOf(entities: Entities, project: string, path: string, branch: string, live: boolean): string {
  const dir = normalize(path);
  for (const worktree of entities.worktrees.values()) {
    if (worktree.project_id !== project || normalize(worktree.path) !== dir) continue;
    return worktree.is_main ? `${worktree.branch} · ${t('agents.newSession.continueMain')}` : worktree.branch;
  }
  return live || !branch ? branch : t('agents.newSession.continueGone', { branch });
}

export function useProjectConversations(project: Ref<string | null>, active: Ref<boolean>): ProjectConversations {
  const berth = useBerth();
  const external = useExternalSessions();
  const listed = shallowRef<OrphanedSession[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let seq = 0;

  watch(
    [project, active],
    async ([id, on]) => {
      if (!id || !on) return;
      const current = ++seq;
      loading.value = true;
      error.value = null;
      try {
        const sessions = await listProjectConversations(id);
        if (current === seq) listed.value = sessions;
      } catch (e) {
        if (current === seq) {
          listed.value = [];
          error.value = describeError(e);
        }
      } finally {
        if (current === seq) loading.value = false;
      }
    },
    { immediate: true },
  );

  const choices = computed<ConversationChoice[]>(() => {
    const entities = berth?.entities.value;
    const id = project.value;
    if (!entities || !id) return [];
    const byId = new Map<string, ConversationChoice>();
    const put = (harness: Harness, sessionId: string, title: string, checkout: string, at: number) => {
      const known = byId.get(sessionId);
      byId.set(sessionId, { key: externalKey({ harness, id: sessionId }), harness, id: sessionId, title: title || known?.title || sessionId.slice(0, 8), checkout: checkout || known?.checkout || '', at: Math.max(at, known?.at ?? 0), busy: null });
    };
    for (const record of listed.value) {
      if (record.project_id !== id || !isHarness(record.kind)) continue;
      put(record.kind, record.session_id, record.name, checkoutOf(entities, id, record.worktree_path, record.branch, record.live), record.orphaned_at || record.created_at);
    }
    for (const session of external?.anyByKey.value.values() ?? []) {
      const place = placeExternal(session.cwd, entities);
      if (place.kind === 'loose' || place.projectId !== id) continue;
      const worktree = place.kind === 'worktree' ? entities.worktrees.get(place.worktreeId) : undefined;
      put(session.harness, session.id, session.title, worktree ? checkoutOf(entities, id, worktree.path, worktree.branch, true) : (session.branch ?? ''), session.updated_at);
    }
    const terminalNames = new Map([...entities.terminals.values()].map((terminal) => [`terminal:${terminal.id}`, terminal.name]));
    for (const agent of entities.agents.values()) {
      if (!agent.session_id || !isHarness(agent.kind)) continue;
      const worktree = entities.worktrees.get(agent.worktree_id);
      if (worktree?.project_id !== id) continue;
      put(agent.kind, agent.session_id, agent.name, checkoutOf(entities, id, worktree.path, worktree.branch, true), 0);
      const choice = byId.get(agent.session_id);
      if (choice && agent.alive) choice.busy = t('agents.newSession.continueRunningIn', { name: agent.name });
    }
    for (const choice of byId.values()) {
      if (choice.busy) continue;
      const session = external?.anyByKey.value.get(choice.key);
      if (!session?.running) continue;
      const host = terminalNames.get(external?.terminalOf.value.get(choice.key) ?? '');
      choice.busy = host ? t('agents.newSession.continueRunningIn', { name: host }) : t('agents.newSession.continueRunningOutside');
    }
    return [...byId.values()].sort((a, b) => b.at - a.at);
  });

  return { choices, loading, error };
}
