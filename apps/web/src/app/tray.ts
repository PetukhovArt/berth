import { watch } from 'vue';
import { useExternalSessions, useBerth } from '../entities/session';
import { invoke, isDesktop, listen, type HostEvent } from '../bridge';
import { openCleanup } from '../features/cleanup';
import { runAction } from '../shared/lib/commands/registry';
import { openUpdate } from '../features/update';
import { startCreateInMain } from '../features/new-session';
import { createScope } from '../shared/lib/scope';

const NEW_SESSION_EVENT = 'tray://new-session';
const UPDATE_EVENT = 'tray://update';
const ROLLBACK_EVENT = 'tray://rollback';
const CLEANUP_EVENT = 'tray://cleanup';
const MENU_ACTION_EVENT = 'menu://action';

export function initTray(): () => void {
  const scope = createScope();
  if (!isDesktop) return () => scope.dispose();
  const on = <T>(event: string, handler: (event: HostEvent<T>) => void) => void listen<T>(event, handler).then((unlisten) => scope.add(unlisten));
  const berth = useBerth();
  const external = useExternalSessions();
  if (berth) {
    scope.add(
      watch(
        () => {
          let running = 0;
          let waiting = 0;
          for (const session of berth.sessions.value.values()) {
            if (session.status === 'running') running += 1;
            else if (session.status === 'needs_feedback') waiting += 1;
          }
          for (const session of [...(external?.sessions.value ?? []), ...(external?.linked.value.values() ?? [])]) {
            if (!session.running) continue;
            if (session.status === 'working') running += 1;
            else if (session.status === 'waiting') waiting += 1;
          }
          return `${running}:${waiting}`;
        },
        (counts) => {
          const [running, waiting] = counts.split(':').map(Number);
          void invoke('tray_status', { running, waiting }).catch(() => undefined);
        },
        { immediate: true },
      ),
    );
  }
  on(NEW_SESSION_EVENT, () => startCreateInMain());
  on(UPDATE_EVENT, () => openUpdate('update'));
  on(ROLLBACK_EVENT, () => openUpdate('rollback'));
  on(CLEANUP_EVENT, openCleanup);
  on<string>(MENU_ACTION_EVENT, (event) => runAction(event.payload));
  return () => scope.dispose();
}
