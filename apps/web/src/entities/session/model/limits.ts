import { computed, shallowRef } from 'vue';
import { hasBackend, invoke } from '../../../bridge';

export type UsageWindow = { kind: string; window_minutes: number; used_percent: number; resets_at_ms: number };

const REFRESH_MS = 60_000;
const SPENT_PERCENT = 100;
const FIVE_HOUR_MINUTES = 300;

const windows = shallowRef<readonly UsageWindow[]>([]);
const clock = shallowRef(Date.now());
let started = false;

function isWindowList(value: unknown): value is UsageWindow[] {
  return Array.isArray(value);
}

async function getUsageLimits(): Promise<UsageWindow[]> {
  const reply = await invoke<unknown>('berth_call', { request: { GetUsageLimits: {} } });
  const body = typeof reply === 'object' && reply !== null && 'UsageLimits' in reply ? (reply as { UsageLimits: { windows: unknown } }).UsageLimits : undefined;
  return body && isWindowList(body.windows) ? body.windows : [];
}

async function refresh(): Promise<void> {
  try {
    windows.value = await getUsageLimits();
  } catch {
    windows.value = [];
  }
  clock.value = Date.now();
}

export function watchUsageLimits(): void {
  if (started || !hasBackend) return;
  started = true;
  void refresh();
  setInterval(() => void refresh(), REFRESH_MS);
}

export function limitUntil(kind: string): number | null {
  const now = clock.value;
  const spent = windows.value.filter((w) => w.kind === kind && w.used_percent >= SPENT_PERCENT && w.resets_at_ms > now);
  return spent.length ? Math.max(...spent.map((w) => w.resets_at_ms)) : null;
}

export function useUsageLimits() {
  watchUsageLimits();
  return { windows: computed(() => windows.value), refresh };
}

export const isFiveHourWindow = (window: Pick<UsageWindow, 'window_minutes'>) => window.window_minutes === FIVE_HOUR_MINUTES;
