import type { SessionStatus } from '../model/berth';
import type { SessionState } from '../model/sessionState';

export type AttentionKind = 'needs_feedback' | 'crashed' | 'limit' | 'unseen';

type Attending = Pick<SessionState, 'status' | 'unseen' | 'alive'>;

const ATTENTION: readonly AttentionKind[] = ['unseen', 'limit', 'crashed', 'needs_feedback'];
const CALM: readonly SessionStatus[] = ['closed', 'disconnected', 'shell', 'fresh', 'finished', 'running'];

export function attentionOf(state: Attending): AttentionKind | null {
  if (state.status === 'needs_feedback') return 'needs_feedback';
  if (state.status === 'terminated' || (state.status === 'disconnected' && !state.alive)) return 'crashed';
  if (state.status === 'limit') return 'limit';
  if (state.status === 'finished' && state.unseen) return 'unseen';
  return null;
}

export function urgencyOf(state: Attending): number {
  const kind = attentionOf(state);
  return kind ? CALM.length + ATTENTION.indexOf(kind) : CALM.indexOf(state.status);
}

export function attentionEdge(previous: SessionStatus, state: Attending): AttentionKind | null {
  if (previous === state.status) return null;
  const kind = attentionOf(state);
  if (kind === 'unseen' && previous !== 'running' && previous !== 'needs_feedback') return null;
  return kind;
}
