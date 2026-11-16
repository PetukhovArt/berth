import { shallowRef } from 'vue';

const KEY = 'berth.firstRun.dismissed';

function read(): boolean {
  try {
    return localStorage.getItem(KEY) === '1';
  } catch {
    return false;
  }
}

const PHONE_KEY = 'berth.firstRun.phoneDismissed';

function readPhone(): boolean {
  try {
    return localStorage.getItem(PHONE_KEY) === '1';
  } catch {
    return false;
  }
}

export const firstRunDismissed = shallowRef(read());
export const phoneRowDismissed = shallowRef(readPhone());

export function dismissPhoneRow(): void {
  phoneRowDismissed.value = true;
  try {
    localStorage.setItem(PHONE_KEY, '1');
  } catch {
    return;
  }
}

export function dismissFirstRun(): void {
  firstRunDismissed.value = true;
  try {
    localStorage.setItem(KEY, '1');
  } catch {
    return;
  }
}
