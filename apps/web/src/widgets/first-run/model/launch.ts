import { computed, shallowRef } from 'vue';
import { hostText } from '../../../shared/lib/i18n/hostText';
import type { useBerth } from '../../../entities/session';

type Berth = NonNullable<ReturnType<typeof useBerth>>;

const GRACE_MS = 8000;

export type Launch = { kind: 'ready' } | { kind: 'starting' } | { kind: 'offline' | 'incompatible'; detail: string | null };

const settled = shallowRef(false);
let timer: ReturnType<typeof setTimeout> | undefined;

function startGrace() {
  timer ??= setTimeout(() => (settled.value = true), GRACE_MS);
}

export function useLaunch(berth: Berth | null) {
  startGrace();
  return computed<Launch>(() => {
    if (!berth || berth.hasConnected.value) return { kind: 'ready' };
    const connection = berth.connection.value;
    if (connection.state === 'connected') return { kind: 'ready' };
    if (connection.state === 'connecting' || !settled.value) return { kind: 'starting' };
    return { kind: connection.state, detail: hostText(connection.message) ?? null };
  });
}
