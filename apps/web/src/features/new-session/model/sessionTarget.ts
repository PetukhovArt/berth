import { computed, shallowRef, watch, type ComputedRef, type ShallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { mrFetchSource } from '../../../shared/api/connectors/api';
import { useSettings } from '../../../shared/config/settings/store';
import { acknowledged, call, describeError, BerthCallError, useBerth, worktreesOfProject, type Branch, type Worktree } from '../../../entities/session';

export type BranchMode = 'current' | 'existing' | 'new';
type TargetPlan = { kind: 'use'; worktree: Worktree } | { kind: 'create'; branch: string; base: string | null; fetch: boolean } | { kind: 'incomplete'; reason: string };
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
  creating: ComputedRef<boolean>;
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

const NEW_WORKTREE = ':new-worktree';
const DEFAULT_BASES = ['develop', 'main', 'master'];

export function useSessionTarget(initialWorktree: string | null): SessionTarget {
  const berth = useBerth();
  const { values } = useSettings();
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

  const targets = computed<Option[]>(() => {
    const listed = worktrees.value.map((worktree) => ({ id: worktree.id, label: worktree.is_main ? t('agents.worktree.main', { branch: worktree.branch }) : worktree.branch }));
    return listed.length ? [...listed, { id: NEW_WORKTREE, label: t('agents.newSession.newWorktree') }] : listed;
  });

  const creating = computed(() => target.value === NEW_WORKTREE);
  const branchMode = computed<BranchMode>(() => (creating.value ? 'new' : mode.value));
  const mainWorktree = computed(() => worktrees.value.find((worktree) => worktree.is_main) ?? worktrees.value[0]);
  const configuredBase = computed(() => {
    const row = project.value ? entities()?.projects.get(project.value) : undefined;
    if (!row) return null;
    const bases = values.value['sessions.worktreeBase'];
    const folder = row.repo_path.split(/[\\/]/).filter(Boolean).at(-1) ?? '';
    const configured = (bases[row.name] ?? bases[folder] ?? '').trim();
    if (configured) return configured;
    const names = new Set(branches.value?.map((branch) => branch.name));
    return DEFAULT_BASES.find((name) => names.has(name)) ?? null;
  });
  const targetWorktree = computed(() => (creating.value ? mainWorktree.value : worktrees.value.find((worktree) => worktree.id === target.value)));

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
        target.value = mainWorktree.value?.id ?? null;
        existing.value = null;
      }
      if (id) void load(id);
    },
    { immediate: true },
  );

  if (!project.value) project.value = projects.value[0]?.id ?? null;

  watch(worktrees, (list) => {
    if (!list.length) target.value = null;
    else if (!creating.value && !list.some((worktree) => worktree.id === target.value)) target.value = mainWorktree.value?.id ?? null;
  }, { immediate: true });

  function planFor(name: string): TargetPlan {
    const all = entities();
    const known = branches.value?.find((branch) => branch.name === name);
    const holder = known?.worktree ? all?.worktrees.get(known.worktree) : worktrees.value.find((worktree) => worktree.branch === name);
    if (holder) return { kind: 'use', worktree: holder };
    if (creating.value && !known && configuredBase.value) return { kind: 'create', branch: name, base: configuredBase.value, fetch: true };
    const base = branchMode.value === 'new' && !known && targetWorktree.value && !isDetached(targetWorktree.value.branch) ? targetWorktree.value.branch : null;
    return { kind: 'create', branch: name, base, fetch: false };
  }

  const plan = computed<TargetPlan>(() => {
    if (!targetWorktree.value) return { kind: 'incomplete', reason: t('agents.newSession.noTarget') };
    if (branchMode.value === 'current') return { kind: 'use', worktree: targetWorktree.value };
    if (branchMode.value === 'existing') return existing.value ? planFor(existing.value) : { kind: 'incomplete', reason: t('agents.newSession.pickBranch') };
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

  async function fetchBase(id: string, branch: string): Promise<string> {
    const row = entities()?.projects.get(id);
    if (!row) throw new BerthCallError(t('agents.newSession.noTarget'));
    const fetched = await mrFetchSource(row.repo_path, branch, '', null);
    if (fetched.problem || !fetched.base) {
      const detail = (fetched.detail ?? fetched.problem ?? '').split(/\r?\n/).filter(Boolean).at(-1) ?? '';
      throw new BerthCallError(t('agents.newSession.baseFetchFailed', { branch, detail }));
    }
    return `${fetched.base}^0`;
  }

  async function resolve(): Promise<string> {
    const current = plan.value;
    if (current.kind === 'incomplete') throw new BerthCallError(current.reason);
    if (current.kind === 'use') return current.worktree.id;
    if (!project.value) throw new BerthCallError(t('agents.newSession.noTarget'));
    const base = current.fetch && current.base ? await fetchBase(project.value, current.base) : current.base;
    const created = await acknowledged({ CreateWorktree: { project: project.value, branch: current.branch, base } });
    if (!created || !('Worktree' in created)) throw new BerthCallError(t('agents.errors.noWorktreeCreated'));
    return created.Worktree;
  }

  return { project, target, mode, existing, fresh, branches, branchesError, loading, projects, targets, creating, plan, takenNames, resolve };
}
