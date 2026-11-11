import { invoke, isDesktop } from '../../../bridge';
import { useSettings } from '../../../shared/config/settings/store';
import { info, quietBuild, refresh } from './state';
import { createScope, visibleInterval } from '../../../shared/lib/scope';

const FIRST_CHECK_MS = 60_000;
const CHECK_MS = 10 * 60_000;

async function check(): Promise<void> {
  const { values } = useSettings();
  if (!values.value['app.autoUpdate']) return;
  await invoke('app_update_fetch').catch(() => undefined);
  await refresh();
  const state = info.value;
  if (!state?.running || !state.head || state.job?.state === 'running') return;
  const installed = state.current?.commit ?? state.running.commit;
  if (state.head.commit === installed) return;
  const stopped = state.job?.state === 'failed' || state.job?.state === 'cancelled';
  if (stopped && state.job?.commit === state.head.commit) return;
  quietBuild.value = true;
  try {
    await invoke('app_update_start');
  } catch {
    quietBuild.value = false;
  }
  await refresh();
}

export function initAutoUpdate(): () => void {
  const scope = createScope();
  if (!isDesktop) return () => scope.dispose();
  const run = () => void check();
  const first = setTimeout(() => {
    run();
    visibleInterval(scope, CHECK_MS, run);
  }, FIRST_CHECK_MS);
  scope.add(() => clearTimeout(first));
  return () => scope.dispose();
}
