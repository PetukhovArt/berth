import type { SessionState } from '../model/sessionState';

export type AttentionKind = 'needs_feedback' | 'crashed' | 'limit' | 'unseen';

type PhaseName = SessionState['phase']['name'];
type Attending = Pick<SessionState, 'phase' | 'alive'>;

const KIND: Readonly<Partial<Record<PhaseName, AttentionKind>>> = { needs_feedback: 'needs_feedback', crashed: 'crashed', limit: 'limit', finished_unseen: 'unseen' };
const ATTENTION: readonly AttentionKind[] = ['unseen', 'limit', 'crashed', 'needs_feedback'];
const CALM: readonly PhaseName[] = ['closed', 'disconnected', 'shell', 'fresh', 'finished', 'running'];
const RAISED_FROM: readonly PhaseName[] = ['running', 'needs_feedback'];

export function attentionOf(state: Attending): AttentionKind | null {
  return KIND[state.phase.name] ?? null;
}

export function urgencyOf(state: Attending): number {
  const kind = attentionOf(state);
  return kind ? CALM.length + ATTENTION.indexOf(kind) : CALM.indexOf(state.phase.name);
}

export function attentionEdge(previous: PhaseName, state: Attending): AttentionKind | null {
  if (previous === state.phase.name) return null;
  const kind = attentionOf(state);
  if (kind === 'unseen' && !RAISED_FROM.includes(previous)) return null;
  return kind;
}
