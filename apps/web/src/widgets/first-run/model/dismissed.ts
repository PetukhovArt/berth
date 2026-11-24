import { shallowRef } from 'vue';

const PHONE_KEY = 'berth.firstRun.phoneDismissed';

function readPhone(): boolean {
  try {
    return localStorage.getItem(PHONE_KEY) === '1';
  } catch {
    return false;
  }
}

export const phoneRowDismissed = shallowRef(readPhone());

export function dismissPhoneRow(): void {
  phoneRowDismissed.value = true;
  try {
    localStorage.setItem(PHONE_KEY, '1');
  } catch {
    return;
  }
}
