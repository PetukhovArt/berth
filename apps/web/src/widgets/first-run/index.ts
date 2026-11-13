import type { Entities } from '../../entities/session';

export { default as FirstRun } from './ui/FirstRun.vue';

export function isFirstRun(entities: Entities | undefined): boolean {
  return !!entities && entities.workspaces.size > 0 && entities.projects.size === 0;
}
