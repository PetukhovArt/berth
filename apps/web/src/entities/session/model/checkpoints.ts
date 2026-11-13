import { acknowledged, BerthCallError, call } from './berth';

export type Checkpoint = { number: number; created_at: number; before_restore: boolean };

export async function listCheckpoints(agent: string): Promise<Checkpoint[]> {
  const reply = await call({ ListCheckpoints: { agent } });
  if (!('Checkpoints' in reply)) throw new BerthCallError(Object.keys(reply).join(''));
  return reply.Checkpoints.checkpoints;
}

export async function restoreCheckpoint(agent: string, number: number): Promise<void> {
  await acknowledged({ RestoreCheckpoint: { agent, number } });
}
