import { shallowRef } from 'vue';
import { navigate } from './nav';

const KEY = 'berth.firstRun.dismissed';

function read(): boolean {
  try {
    return localStorage.getItem(KEY) === '1';
  } catch {
    return false;
  }
}

export const firstRunActive = shallowRef(false);
export const firstRunDismissed = shallowRef(read());

export function dismissFirstRun(): void {
  firstRunDismissed.value = true;
  try {
    localStorage.setItem(KEY, '1');
  } catch {
    return;
  }
}

export function reopenFirstRun(): void {
  void navigate({ kind: 'agents' });
  firstRunDismissed.value = false;
  try {
    localStorage.removeItem(KEY);
  } catch {
    return;
  }
}
