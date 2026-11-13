import { watch } from 'vue';
import { isDesktop } from '../../../bridge';
import { lookupMr } from '../../../entities/merge-request';
import { archiveAgent, useBerth } from '../../../entities/session';
import { useSettings } from '../../../shared/config/settings/store';
import { createScope, visibleInterval } from '../../../shared/lib/scope';

const STORE_KEY = 'berth.archiveOnMerge.done';
const CHECK_EVERY_MS = 5 * 60 * 1000;
const LIMIT = 500;

function loadDone(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]');
    return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
  } catch {
    return [];
  }
}

export function initArchiveOnMerge(): () => void {
  const scope = createScope();
  const berth = useBerth();
  if (!isDesktop || !berth) return () => scope.dispose();
  const { values } = useSettings();
  const done = new Set(loadDone());
  let running = false;

  const remember = (id: string) => {
    done.add(id);
    localStorage.setItem(STORE_KEY, JSON.stringify([...done].slice(-LIMIT)));
  };

  const sweep = async () => {
    if (running || !values.value['sessions.archiveOnMerge'] || berth.connection.value.state !== 'connected') return;
    running = true;
    try {
      const { agents, worktrees } = berth.entities.value;
      for (const worktree of worktrees.values()) {
        if (worktree.is_main || !worktree.branch) continue;
        const idle = [...agents.values()].filter(
          (agent) => agent.worktree_id === worktree.id && !agent.archived && !done.has(agent.id) && agent.status !== 'running' && agent.status !== 'needs_feedback',
        );
        if (!idle.length) continue;
        const mr = await lookupMr(worktree.path, worktree.branch).catch(() => null);
        if (scope.disposed || !mr || mr.state !== 'merged') continue;
        for (const agent of idle) {
          remember(agent.id);
          await archiveAgent(agent.id).catch(() => undefined);
        }
      }
    } finally {
      running = false;
    }
  };

  scope.add(watch(() => [values.value['sessions.archiveOnMerge'], berth.connection.value.state] as const, () => void sweep(), { immediate: true }));
  visibleInterval(scope, CHECK_EVERY_MS, () => void sweep());
  return () => scope.dispose();
}
