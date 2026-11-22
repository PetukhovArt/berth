import { ref, shallowRef } from 'vue';
import { invoke, isDesktop } from '../../../bridge';
import { settleAll } from '../../../entities/document';
import { hostPlatform } from '../../../shared/lib/platform';

export type ReleaseStatus =
  | { state: 'idle' | 'checking' | 'upToDate' }
  | { state: 'downloading'; version: string; downloaded: number; total: number | null }
  | { state: 'ready'; version: string; notes: string | null }
  | { state: 'installing'; version: string }
  | { state: 'failed'; error: string };

const POLL_MS = 500;

export const releaseStatus = shallowRef<ReleaseStatus>({ state: 'idle' });
export const releaseOpen = ref(false);
export const releaseFailure = ref<string | null>(null);

let checking: Promise<void> | null = null;

export const releaseUpdates = () => isDesktop && hostPlatform.value.releaseUpdate;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function poll(): Promise<void> {
  releaseStatus.value = await invoke<ReleaseStatus>('app_release_status').catch(() => releaseStatus.value);
}

export function checkRelease(): Promise<void> {
  checking ??= (async () => {
    const poller = setInterval(() => void poll(), POLL_MS);
    try {
      releaseStatus.value = await invoke<ReleaseStatus>('app_release_check');
    } catch (error) {
      releaseStatus.value = { state: 'failed', error: message(error) };
    } finally {
      clearInterval(poller);
      checking = null;
    }
  })();
  return checking;
}

export async function installRelease(): Promise<void> {
  if (!(await settleAll())) return;
  releaseFailure.value = null;
  const poller = setInterval(() => void poll(), POLL_MS);
  try {
    await invoke('app_release_install');
  } catch (error) {
    releaseFailure.value = message(error);
    releaseOpen.value = true;
  } finally {
    clearInterval(poller);
    await poll();
  }
}

export function openRelease(): void {
  releaseFailure.value = null;
  releaseOpen.value = true;
  void checkRelease();
}

export function closeRelease(): void {
  releaseOpen.value = false;
}
