import { acknowledged, BerthCallError, call } from './berth';

export async function loadProjectSetup(project: string): Promise<string> {
  const reply = await call({ GetProjectSetup: { project } });
  if (!('ProjectSetup' in reply)) throw new BerthCallError(Object.keys(reply).join(''));
  return reply.ProjectSetup.command;
}

export async function saveProjectSetup(project: string, command: string): Promise<void> {
  await acknowledged({ SetProjectSetup: { project, command } });
}
