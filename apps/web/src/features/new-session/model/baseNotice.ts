import { shallowRef } from 'vue';

export const baseNotice = shallowRef<string | null>(null);

export function showBaseNotice(text: string): void {
  baseNotice.value = text;
}

export function dismissBaseNotice(): void {
  baseNotice.value = null;
}
