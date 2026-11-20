import { t } from '../../../shared/lib/i18n/i18n';
import { acknowledged, BerthCallError, call } from './berth';

export async function loadProjectSetup(project: string): Promise<string> {
  const reply = await call({ GetProjectSetup: { project } });
  if (!('ProjectSetup' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ProjectSetup.command;
}

export async function saveProjectSetup(project: string, command: string): Promise<void> {
  await acknowledged({ SetProjectSetup: { project, command } });
}
