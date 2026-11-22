import type { SessionStatus } from '../model/berth';
import type { SessionState } from '../model/sessionState';

const STOPPABLE: readonly SessionStatus[] = ['running', 'finished'];

export function isRunning(state: Pick<SessionState, 'status' | 'alive'>): boolean {
  return state.status === 'running' && state.alive;
}

export function isStopped(status: SessionStatus | undefined, unseen: boolean | undefined, alive: boolean | undefined): boolean {
  return alive === false && !unseen && status !== undefined && STOPPABLE.includes(status);
}
