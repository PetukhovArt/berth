import { shallowRef } from 'vue';
import { invoke } from '../../../bridge';
import { itemId } from '../../../entities/merge-request';
import type { ChangeRequestRef } from '../../../shared/api/connectors/api';

const LEGACY_KEY = 'berth.watchedMergeRequests';

type Watched = ChangeRequestRef & { last_wake: string | null };

const watched = shallowRef<readonly Watched[]>([]);

const find = (target: ChangeRequestRef) => watched.value.find((entry) => itemId(entry) === itemId(target));

export const isWatching = (target: ChangeRequestRef): boolean => !!find(target);

export const lastWake = (target: ChangeRequestRef): string | null => find(target)?.last_wake ?? null;

async function setWatch(target: ChangeRequestRef, on: boolean) {
  watched.value = await invoke<Watched[]>('merge_watch_set', { connector: target.connector, project: target.project, number: target.number, on });
}

export const stopWatching = (target: ChangeRequestRef) => setWatch(target, false);

export const toggleWatch = (target: ChangeRequestRef) => setWatch(target, !isWatching(target));

export async function refreshWatches() {
  watched.value = await invoke<Watched[]>('merge_watch_list');
}

function legacyWatches(): ChangeRequestRef[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(LEGACY_KEY) ?? '[]');
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is ChangeRequestRef => !!item && typeof item.connector === 'string' && typeof item.project === 'string' && typeof item.number === 'number');
  } catch {
    return [];
  }
}

export async function loadWatchedMergeRequests() {
  try {
    await refreshWatches();
    const legacy = legacyWatches().filter((target) => !isWatching(target));
    for (const target of legacy) await setWatch(target, true);
    localStorage.removeItem(LEGACY_KEY);
  } catch {
    return;
  }
}
