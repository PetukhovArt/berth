import { computed, shallowRef, watch } from 'vue';
import { isDesktop } from '../../../bridge';
import { useBerth } from '../../../entities/session';
import { worktreeChanges } from '../../../shared/api/fs';
import { createScope, visibleInterval } from '../../../shared/lib/scope';

type Overlap = { names: string[]; modules: string[] };

type Member = { id: string; name: string; worktree: string; project: string; path: string };

const REFRESH_MS = 30_000;
const MODULE_DEPTH = 2;

const modulesByPath = shallowRef<ReadonlyMap<string, ReadonlySet<string>>>(new Map());

function moduleOf(file: string): string {
  const parts = file.split('/').filter(Boolean);
  return parts.length <= 1 ? parts.join('/') : parts.slice(0, Math.min(MODULE_DEPTH, parts.length - 1)).join('/');
}

function findOverlaps(members: Member[], modules: ReadonlyMap<string, ReadonlySet<string>>): Map<string, Overlap> {
  const result = new Map<string, Overlap>();
  for (const member of members) {
    const mine = modules.get(member.path);
    if (!mine?.size) continue;
    const names = new Set<string>();
    const shared = new Set<string>();
    for (const other of members) {
      if (other.project !== member.project || other.worktree === member.worktree) continue;
      for (const module of modules.get(other.path) ?? []) {
        if (!mine.has(module)) continue;
        names.add(other.name);
        shared.add(module);
      }
    }
    if (names.size) result.set(member.id, { names: [...names], modules: [...shared].sort() });
  }
  return result;
}

function membersOf(): Member[] {
  const berth = useBerth();
  if (!berth) return [];
  const { agents, worktrees } = berth.entities.value;
  const list: Member[] = [];
  for (const agent of agents.values()) {
    if (agent.archived) continue;
    const worktree = worktrees.get(agent.worktree_id);
    if (worktree) list.push({ id: agent.id, name: agent.name, worktree: worktree.id, project: worktree.project_id, path: worktree.path });
  }
  return list;
}

const overlaps = computed(() => findOverlaps(membersOf(), modulesByPath.value));

export const useOverlaps = () => overlaps;

export function initOverlaps(): () => void {
  const scope = createScope();
  const berth = useBerth();
  if (!isDesktop || !berth) return () => scope.dispose();
  let running = false;

  const refresh = async () => {
    if (running) return;
    running = true;
    try {
      const paths = [...new Set(membersOf().map((member) => member.path))];
      const next = new Map<string, ReadonlySet<string>>();
      await Promise.all(
        paths.map(async (path) => {
          try {
            const files = await worktreeChanges(path);
            next.set(path, new Set(files.map((file) => moduleOf(file.new_path || file.old_path))));
          } catch {
            const known = modulesByPath.value.get(path);
            if (known) next.set(path, known);
          }
        }),
      );
      if (!scope.disposed) modulesByPath.value = next;
    } finally {
      running = false;
    }
  };

  const signature = computed(() =>
    membersOf()
      .map((member) => member.path)
      .sort()
      .join('\n'),
  );
  scope.add(watch([signature, () => berth.connection.value.state], () => void refresh(), { immediate: true }));
  visibleInterval(scope, REFRESH_MS, () => void refresh());
  return () => scope.dispose();
}
