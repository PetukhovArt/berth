import { computed, type ComputedRef } from 'vue';
import { sendInput, useBerth, type FeedbackRequest, type SessionRef } from './berth';

const ALLOW = '1';
const DENY = '\x1b';

export function useFeedbackRequest(session: SessionRef): ComputedRef<FeedbackRequest | undefined> {
  return computed(() => ('Agent' in session ? useBerth()?.feedback.value.get(session.Agent) : undefined));
}

export function allowRequest(session: SessionRef): Promise<void> {
  return sendInput(session, ALLOW);
}

export function denyRequest(session: SessionRef): Promise<void> {
  return sendInput(session, DENY);
}

export function chooseOption(session: SessionRef, index: number): Promise<void> {
  return sendInput(session, String(index + 1));
}
