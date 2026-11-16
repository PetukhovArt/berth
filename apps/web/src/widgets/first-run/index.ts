import type { Entities } from '../../entities/session';
import { firstRunDismissed } from './model/dismissed';

export { default as FirstRun } from './ui/FirstRun.vue';

export function isFirstRun(entities: Entities | undefined): boolean {
  return !firstRunDismissed.value && !!entities && entities.workspaces.size > 0 && entities.projects.size === 0;
}
