import { inject, provide, type InjectionKey } from 'vue';
import type { ChangeRequestStore } from './changeRequest';

const KEY: InjectionKey<ChangeRequestStore> = Symbol('change-request');

export function provideChangeRequest(store: ChangeRequestStore): void {
  provide(KEY, store);
}

export function useChangeRequest(): ChangeRequestStore {
  const store = inject(KEY);
  if (!store) throw new Error('useChangeRequest() outside ChangeRequestView');
  return store;
}
