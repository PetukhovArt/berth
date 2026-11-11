import { watch } from 'vue';
import { useExternalSessions, useBerth } from '../entities/session';
import { invoke, isDesktop, listen } from '../bridge';
import { openCleanup } from '../features/cleanup';
import { runAction } from '../shared/lib/commands/registry';
import { openUpdate } from '../features/update';
import { startCreateInMain } from '../features/new-session';

const NEW_SESSION_EVENT = 'tray://new-session';
const UPDATE_EVENT = 'tray://update';
const ROLLBACK_EVENT = 'tray://rollback';
const CLEANUP_EVENT = 'tray://cleanup';
const MENU_ACTION_EVENT = 'menu://action';

export function installTray(): void {
  if (!isDesktop) return;
  const berth = useBerth();
  const external = useExternalSessions();
  if (berth) {
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
    );
  }
  void listen(NEW_SESSION_EVENT, () => startCreateInMain());
  void listen(UPDATE_EVENT, () => openUpdate('update'));
  void listen(ROLLBACK_EVENT, () => openUpdate('rollback'));
  void listen(CLEANUP_EVENT, openCleanup);
  void listen<string>(MENU_ACTION_EVENT, (event) => runAction(event.payload));
}
