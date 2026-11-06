import { computed, shallowRef, watch, type ComputedRef, type ShallowRef } from 'vue';
import { t } from '../../i18n';
import { acknowledged, call, describeError, BerthCallError, useBerth, worktreesOfProject, type Branch, type Worktree } from '../berth';

export type BranchMode = 'current' | 'existing' | 'new';
type TargetPlan = { kind: 'use'; worktree: Worktree } | { kind: 'create'; branch: string; base: string | null } | { kind: 'incomplete'; reason: string };
type Option = { id: string; label: string };

export type SessionTarget = {
  project: ShallowRef<string | null>;
  target: ShallowRef<string | null>;
  mode: ShallowRef<BranchMode>;
  existing: ShallowRef<string | null>;
  fresh: ShallowRef<string>;
  branches: ShallowRef<Branch[] | null>;
  branchesError: ShallowRef<string | null>;
  loading: ShallowRef<boolean>;
  projects: ComputedRef<Option[]>;
  targets: ComputedRef<Option[]>;
  plan: ComputedRef<TargetPlan>;
  takenNames: ComputedRef<ReadonlySet<string>>;
  resolve: () => Promise<string>;
};

async function listBranches(project: string): Promise<Branch[]> {
  const reply = await call({ ListBranches: { project } });
  if (!('Branches' in reply)) throw new BerthCallError(t('agents.errors.expectedBranches', { reply: Object.keys(reply).join('') }));
  return reply.Branches.branches;
}

const isDetached = (branch: string) => branch.startsWith('detached');

export function useSessionTarget(initialWorktree: string | null): SessionTarget {
  const berth = useBerth();
  const entities = () => berth?.entities.value;
  const initial = initialWorktree ? entities()?.worktrees.get(initialWorktree) : undefined;

  const project = shallowRef<string | null>(initial?.project_id ?? null);
  const target = shallowRef<string | null>(initial?.id ?? null);
  const mode = shallowRef<BranchMode>('current');
  const existing = shallowRef<string | null>(null);
  const fresh = shallowRef('');
  const branches = shallowRef<Branch[] | null>(null);
  const branchesError = shallowRef<string | null>(null);
  const loading = shallowRef(false);

  const projects = computed<Option[]>(() =>
    (berth?.tree.value ?? []).flatMap((workspace) => workspace.projects.map((candidate) => ({ id: candidate.id, label: candidate.name }))),
  );

  const worktrees = computed(() => {
    const all = entities();
    return all && project.value ? worktreesOfProject(all, project.value) : [];
  });

  const targets = computed<Option[]>(() =>
    worktrees.value.map((worktree) => ({ id: worktree.id, label: worktree.is_main ? t('agents.worktree.main', { branch: worktree.branch }) : worktree.branch })),
  );

  const targetWorktree = computed(() => worktrees.value.find((worktree) => worktree.id === target.value));

  let loadSeq = 0;
  async function load(id: string) {
    const seq = ++loadSeq;
    loading.value = true;
    branches.value = null;
    branchesError.value = null;
    try {
      const listed = await listBranches(id);
      if (seq === loadSeq) branches.value = listed;
    } catch (error) {
      if (seq === loadSeq) branchesError.value = describeError(error);
    } finally {
      if (seq === loadSeq) loading.value = false;
    }
  }

  watch(
    project,
    (id, previous) => {
      if (previous !== undefined) {
        const list = worktrees.value;
        target.value = (list.find((worktree) => worktree.is_main) ?? list[0])?.id ?? null;
        existing.value = null;
      }
      if (id) void load(id);
    },
    { immediate: true },
  );

  if (!project.value) project.value = projects.value[0]?.id ?? null;

  watch(worktrees, (list) => {
    if (!list.some((worktree) => worktree.id === target.value)) target.value = (list.find((worktree) => worktree.is_main) ?? list[0])?.id ?? null;
  });

  function planFor(name: string): TargetPlan {
    const all = entities();
    const known = branches.value?.find((branch) => branch.name === name);
    const holder = known?.worktree ? all?.worktrees.get(known.worktree) : worktrees.value.find((worktree) => worktree.branch === name);
    if (holder) return { kind: 'use', worktree: holder };
    const base = mode.value === 'new' && !known && targetWorktree.value && !isDetached(targetWorktree.value.branch) ? targetWorktree.value.branch : null;
    return { kind: 'create', branch: name, base };
  }

  const plan = computed<TargetPlan>(() => {
    if (!targetWorktree.value) return { kind: 'incomplete', reason: t('agents.newSession.noTarget') };
    if (mode.value === 'current') return { kind: 'use', worktree: targetWorktree.value };
    if (mode.value === 'existing') return existing.value ? planFor(existing.value) : { kind: 'incomplete', reason: t('agents.newSession.pickBranch') };
    const name = fresh.value.trim();
    if (!name) return { kind: 'incomplete', reason: t('agents.newSession.typeBranch') };
    if (/\s/.test(name)) return { kind: 'incomplete', reason: t('agents.newSession.badBranch') };
    return planFor(name);
  });

  const takenNames = computed<ReadonlySet<string>>(() => {
    const current = plan.value;
    const all = entities();
    if (current.kind !== 'use' || !all) return new Set();
    const id = current.worktree.id;
    return new Set([
      ...[...all.agents.values()].filter((agent) => agent.worktree_id === id && !agent.archived).map((agent) => agent.name),
      ...[...all.terminals.values()].filter((terminal) => terminal.worktree_id === id).map((terminal) => terminal.name),
    ]);
  });

  async function resolve(): Promise<string> {
    const current = plan.value;
    if (current.kind === 'incomplete') throw new BerthCallError(current.reason);
    if (current.kind === 'use') return current.worktree.id;
    if (!project.value) throw new BerthCallError(t('agents.newSession.noTarget'));
    const created = await acknowledged({ CreateWorktree: { project: project.value, branch: current.branch, base: current.base } });
    if (!created || !('Worktree' in created)) throw new BerthCallError(t('agents.errors.noWorktreeCreated'));
    return created.Worktree;
  }

  return { project, target, mode, existing, fresh, branches, branchesError, loading, projects, targets, plan, takenNames, resolve };
}
