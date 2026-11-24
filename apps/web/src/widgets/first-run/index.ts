import type { Entities } from '../../entities/session';
import { firstRunDismissed } from '../../shared/model/firstRun';

export { default as FirstRun } from './ui/FirstRun.vue';
export { default as LaunchScreen } from './ui/LaunchScreen.vue';
export { useLaunch } from './model/launch';

export function isFirstRun(entities: Entities | undefined): boolean {
  return !firstRunDismissed.value && !!entities && entities.workspaces.size > 0 && entities.projects.size === 0;
}
