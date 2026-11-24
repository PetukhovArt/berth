import { computed } from 'vue';
import { isActive, sessionKey, useBerth, type SessionNode } from '../../../entities/session';
import { itemId, knownMr } from '../../../entities/merge-request';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';

export function useMrSession(target: () => ChangeRequestRef) {
  const berth = useBerth();
  return computed<SessionNode | null>(() => {
    if (!berth) return null;
    const id = itemId(target());
    const { worktrees, agents } = berth.entities.value;
    for (const worktree of worktrees.values()) {
      const mr = knownMr(worktree.path, worktree.branch);
      if (!mr || itemId(mr) !== id) continue;
      const agent = [...agents.values()]
        .filter((candidate) => candidate.worktree_id === worktree.id && isActive(candidate))
        .sort((a, b) => Number(b.alive) - Number(a.alive) || b.status_changed_at - a.status_changed_at)[0];
      const node = agent ? berth.sessions.value.get(sessionKey({ Agent: agent.id })) : undefined;
      if (node) return node;
    }
    return null;
  });
}

export function useMrWorktree(target: () => ChangeRequestRef) {
  const berth = useBerth();
  return computed(() => {
    if (!berth) return null;
    const id = itemId(target());
    for (const worktree of berth.entities.value.worktrees.values()) {
      const mr = knownMr(worktree.path, worktree.branch);
      if (mr && itemId(mr) === id) return worktree;
    }
    return null;
  });
}
