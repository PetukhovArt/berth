import { shallowRef } from 'vue';
import { navigate, view } from '../../../shared/model/nav';
import { useBerth, worktreesOfProject, type AcpAttachmentUpload, type SessionRef, useSessionDock } from '../../../entities/session';

export type Launch = { prompt?: string; uploads?: AcpAttachmentUpload[]; autoStart?: boolean };

export const creatingIn = shallowRef<({ worktree: string | null; projectLocked: boolean } & Launch) | null>(null);

export function startCreate(worktreeId: string | null, projectLocked = false, launch: Launch = {}): void {
  if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
  creatingIn.value = { worktree: worktreeId, projectLocked, ...launch };
}

export function defaultMainTarget() {
  const berth = useBerth();
  const entities = berth?.entities.value;
  const active = useSessionDock().activeWorktreeId.value;
  const project = (active ? entities?.worktrees.get(active)?.project_id : undefined) ?? berth?.tree.value.flatMap((workspace) => workspace.projects)[0]?.id;
  if (!entities || !project) return undefined;
  const worktrees = worktreesOfProject(entities, project);
  const worktree = worktrees.find((candidate) => candidate.is_main) ?? worktrees[0];
  const owner = entities.projects.get(project);
  return worktree && owner ? { worktree, project: owner } : undefined;
}

export function startCreateInMain(launch: Launch = {}): void {
  startCreate(defaultMainTarget()?.worktree.id ?? null, false, launch);
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
