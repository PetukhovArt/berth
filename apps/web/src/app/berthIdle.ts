import { watch } from 'vue';
import { hasBackend, invoke } from '../bridge';
import { SETTINGS, type SettingKey } from '../shared/config/settings/schema';
import { useSettings } from '../shared/config/settings/store';
import { createScope } from '../shared/lib/scope';

const IDLE_CHOICES = new Set<unknown>(SETTINGS['sessions.unloadIdleAfter'].options.map((option) => option.value));

const SYNCED: { key: SettingKey; command: string; accepts: (value: unknown) => boolean }[] = [
  { key: 'sessions.unloadIdleAfter', command: 'berth_idle_timeout', accepts: (value) => IDLE_CHOICES.has(value) },
  { key: 'sessions.restoreOnRestart', command: 'berth_restore_sessions', accepts: (value) => typeof value === 'boolean' },
];

export function initBerthIdleSync(): () => void {
  const scope = createScope();
  if (!hasBackend) return () => scope.dispose();
  const { loaded, values, set } = useSettings();

  const pulls = SYNCED.map(({ key, command, accepts }) => {
    let known: unknown = null;

    scope.add(
      watch(
        () => values.value[key],
        async (next) => {
          if (known === null || next === known) return;
          try {
            await scope.load(invoke<void>(`${command}_write`, { value: next }));
            known = next;
          } catch {
            return;
          }
        },
      ),
    );

    return async (): Promise<void> => {
      try {
        const remote = await scope.load(invoke<unknown>(`${command}_read`));
        if (!accepts(remote)) return;
        known = remote;
        if (values.value[key] !== remote) set(key, remote);
      } catch {
        return;
      }
    };
  });

  const pull = () => pulls.forEach((run) => void run());

  let started = false;
  scope.add(
    watch(
      loaded,
      (ready) => {
        if (!ready || started) return;
        started = true;
        pull();
        window.addEventListener('focus', pull);
        scope.add(() => window.removeEventListener('focus', pull));
      },
      { immediate: true },
    ),
  );
  return () => scope.dispose();
}
