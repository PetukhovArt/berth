import { shallowRef, type ShallowRef } from 'vue';
import { hasBackend, invoke } from '../../bridge';

export type Provider = { id: string; name: string; acp: boolean; enabled: boolean; command: string | null; found: boolean | null };

const providers = shallowRef<readonly Provider[]>([]);
let loading: Promise<void> | null = null;

export function reloadProviders(): Promise<void> {
  if (!hasBackend) return Promise.resolve();
  loading = invoke<Provider[]>('providers_read')
    .then((rows) => {
      providers.value = rows;
    })
    .catch(() => {
      providers.value = [];
    });
  return loading;
}

export function useProviders(): ShallowRef<readonly Provider[]> {
  if (!loading) void reloadProviders();
  return providers;
}

export async function setProviderEnabled(provider: Provider, enabled: boolean): Promise<void> {
  await invoke<void>('providers_write', { id: provider.id, acp: provider.acp, enabled });
  await reloadProviders();
}
