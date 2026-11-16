import { shallowRef } from 'vue';

const KEY = 'berth.firstRun.dismissed';

function read(): boolean {
  try {
    return localStorage.getItem(KEY) === '1';
  } catch {
    return false;
  }
}

export const firstRunDismissed = shallowRef(read());

export function dismissFirstRun(): void {
  firstRunDismissed.value = true;
  try {
    localStorage.setItem(KEY, '1');
  } catch {
    return;
  }
}
