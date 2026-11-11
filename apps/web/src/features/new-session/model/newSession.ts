import { shallowRef } from 'vue';
import { navigate, view } from '../../../shared/model/nav';
import { useBerth, worktreesOfProject, type SessionRef } from '../../../entities/session/model/berth';
import { useSessionDock } from '../../../entities/session/model/sessionDock';

export const creatingIn = shallowRef<{ worktree: string | null } | null>(null);

export function startCreate(worktreeId: string | null): void {
  if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
  creatingIn.value = { worktree: worktreeId };
}

export function startCreateInMain(): void {
  const berth = useBerth();
  const entities = berth?.entities.value;
  const active = useSessionDock().activeWorktreeId.value;
  const project = (active ? entities?.worktrees.get(active)?.project_id : undefined) ?? berth?.tree.value.flatMap((workspace) => workspace.projects)[0]?.id;
  const target = entities && project ? (worktreesOfProject(entities, project).find((worktree) => worktree.is_main) ?? worktreesOfProject(entities, project)[0]) : undefined;
  startCreate(target?.id ?? null);
}

export function onCreated(session: SessionRef): void {
  creatingIn.value = null;
  useSessionDock().openWhenListed(session);
}

export function cancelCreate(): void {
  creatingIn.value = null;
}

export function startNewSession(): void {
  const active = useSessionDock().activeWorktreeId.value;
  if (active) startCreate(active);
  else startCreateInMain();
}
