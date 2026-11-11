import { computed, ref, watch } from 'vue';
import type { MessageKey } from '../../../shared/lib/i18n/i18n';
import { info, type Job } from './state';

const STAGES =['start', 'export', 'deps', 'tokens', 'app', 'daemon', 'install'] as const;
type Stage =(typeof STAGES)[number];

export const STAGE_LABELS = {
  start: 'update.steps.start',
  export: 'update.steps.export',
  deps: 'update.steps.deps',
  tokens: 'update.steps.tokens',
  app: 'update.steps.app',
  daemon: 'update.steps.daemon',
  install: 'update.steps.install',
} as const satisfies Record<Stage, MessageKey>;

const DEFAULT_MS: Record<Stage, number> = { start: 2000, export: 5000, deps: 5000, tokens: 3000, app: 270000, daemon: 60000, install: 5000 };
const STORE_KEY = 'berth.update.durations';

type StageState = 'done' | 'running' | 'pending' | 'skipped';
type StageRow ={ stage: Stage; state: StageState; ms: number };

function loadEstimates(): Record<Stage, number> {
  const result = { ...DEFAULT_MS };
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? 'null');
    if (stored && typeof stored === 'object') {
      for (const stage of STAGES) {
        const value = (stored as Record<string, unknown>)[stage];
        if (typeof value === 'number' && value > 0) result[stage] = value;
      }
    }
  } catch {
    return result;
  }
  return result;
}

const estimates = ref(loadEstimates());
const seen = ref<Record<string, number>>({});
let seenFor: string | undefined;

export const now = ref(Date.now());

function time(value: string | undefined): number | undefined {
  const parsed = value ? Date.parse(value) : NaN;
  return Number.isNaN(parsed) ? undefined : parsed;
}

function starts(job: Job): Partial<Record<Stage, number>> {
  const result: Partial<Record<Stage, number>> = {};
  for (const stage of STAGES) {
    const at = time(job.steps?.[stage]) ?? seen.value[stage];
    if (at !== undefined) result[stage] = at;
  }
  result.start ??= time(job.startedAt);
  return result;
}

function durations(job: Job, end: number): Partial<Record<Stage, number>> {
  const at = starts(job);
  const result: Partial<Record<Stage, number>> = {};
  STAGES.forEach((stage, index) => {
    const begin = at[stage];
    if (begin === undefined) return;
    const next = STAGES.slice(index + 1)
      .map((later) => at[later])
      .find((value) => value !== undefined);
    result[stage] = Math.max(0, (next ?? end) - begin);
  });
  return result;
}

watch(
  () => info.value?.job ?? null,
  (job, previous) => {
    if (job?.state === 'running') {
      if (job.startedAt !== seenFor) {
        seenFor = job.startedAt;
        seen.value = {};
      }
      const step = job.step ?? 'start';
      if (!(step in seen.value)) seen.value = { ...seen.value, [step]: Date.now() };
      return;
    }
    if (job?.state !== 'done' || job.upToDate || previous?.state !== 'running') return;
    const measured = durations(job, time(job.finishedAt) ?? Date.now());
    const next = { ...estimates.value };
    for (const stage of STAGES) {
      const value = measured[stage];
      if (value && stage !== 'start') next[stage] = value;
    }
    estimates.value = next;
    localStorage.setItem(STORE_KEY, JSON.stringify(next));
  },
);

export const rows = computed<StageRow[]>(() => {
  const job = info.value?.job;
  if (!job) return [];
  const current = STAGES.indexOf((job.step ?? 'start') as Stage);
  const measured = durations(job, now.value);
  return STAGES.map((stage, index) => {
    const ms = measured[stage];
    if (index === current) return { stage, state: 'running', ms: ms ?? 0 };
    if (index < current) return ms === undefined ? { stage, state: 'skipped', ms: 0 } : { stage, state: 'done', ms };
    return { stage, state: 'pending', ms: estimates.value[stage] };
  });
});

export const totals = computed(() => {
  let elapsed = 0;
  let left = 0;
  for (const row of rows.value) {
    if (row.state === 'done') elapsed += row.ms;
    if (row.state === 'running') {
      elapsed += row.ms;
      left += Math.max(estimates.value[row.stage] - row.ms, 0);
    }
    if (row.state === 'pending') left += row.ms;
  }
  const progress = elapsed + left > 0 ? Math.min(99, Math.max(2, Math.round((elapsed / (elapsed + left)) * 100))) : 2;
  return { elapsed, left, progress };
});

export function clock(ms: number): string {
  const seconds = Math.round(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
