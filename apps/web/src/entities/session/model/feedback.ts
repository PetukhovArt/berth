import { computed, type ComputedRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { answerFeedback, isAcpAgent, sendInput, useBerth, type FeedbackRequest, type SessionRef } from './berth';

export const ALLOW = '1';
export const DENY = '\x1b';

export function useFeedbackRequest(session: SessionRef): ComputedRef<FeedbackRequest | undefined> {
  return computed(() => ('Agent' in session ? useBerth()?.feedback.value.get(session.Agent) : undefined));
}

export function feedbackText(request: FeedbackRequest): string {
  return request.kind === 'Permission' ? t('feedback.permission', { tool: request.title }) : request.title;
}

export function allowRequest(session: SessionRef): Promise<void> {
  return sendInput(session, ALLOW);
}

export function denyRequest(session: SessionRef): Promise<void> {
  return sendInput(session, DENY);
}

export function chooseOption(session: SessionRef, index: number): Promise<void> {
  if ('Agent' in session && isAcpAgent(session.Agent)) return answerFeedback(session.Agent, index);
  return sendInput(session, String(index + 1));
}
