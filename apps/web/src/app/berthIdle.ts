import { watch } from 'vue';
import { hasBackend, invoke } from '../bridge';
import { SETTINGS, checkValue, type SettingKey } from '../shared/config/settings/schema';
import { useSettings } from '../shared/config/settings/store';
import { createScope } from '../shared/lib/scope';

const IDLE_CHOICES = new Set<unknown>(SETTINGS['sessions.unloadIdleAfter'].options.map((option) => option.value));

const isBoolean = (value: unknown) => typeof value === 'boolean';
const same = (a: unknown, b: unknown) => a === b || JSON.stringify(a) === JSON.stringify(b);

const SYNCED: { key: SettingKey; command: string; args?: Record<string, unknown>; accepts: (value: unknown) => boolean }[] = [
  { key: 'sessions.unloadIdleAfter', command: 'berth_idle_timeout', accepts: (value) => IDLE_CHOICES.has(value) },
  { key: 'sessions.restoreOnRestart', command: 'berth_restore_sessions', accepts: isBoolean },
  { key: 'daemon.gitInitOnCreate', command: 'berth_flag', args: { key: 'git_init_on_create' }, accepts: isBoolean },
  { key: 'daemon.prewarm', command: 'berth_flag', args: { key: 'prewarm' }, accepts: isBoolean },
  { key: 'daemon.claudeLimits', command: 'berth_flag', args: { key: 'claude_limits' }, accepts: isBoolean },
  { key: 'daemon.checkpointsClaude', command: 'berth_flag', args: { key: 'checkpoints_claude' }, accepts: isBoolean },
  { key: 'sessions.worktreeBase', command: 'berth_worktree_base', accepts: (value) => checkValue(SETTINGS['sessions.worktreeBase'], value) === null },
];

export function initBerthIdleSync(): () => void {
  const scope = createScope();
  if (!hasBackend) return () => scope.dispose();
  const { loaded, values, set } = useSettings();

  const pulls = SYNCED.map(({ key, command, args, accepts }) => {
    let known: unknown = null;

    scope.add(
      watch(
        () => values.value[key],
        async (next) => {
          if (known === null || same(next, known)) return;
          try {
            await scope.load(invoke<void>(`${command}_write`, { ...args, value: next }));
            known = next;
          } catch {
            return;
          }
        },
      ),
    );

    return async (): Promise<void> => {
      try {
        const remote = await scope.load(invoke<unknown>(`${command}_read`, args));
        if (remote === null) {
          const local = values.value[key];
          await scope.load(invoke<void>(`${command}_write`, { ...args, value: local }));
          known = local;
          return;
        }
        if (!accepts(remote)) return;
        known = remote;
        if (!same(values.value[key], remote)) set(key, remote);
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
