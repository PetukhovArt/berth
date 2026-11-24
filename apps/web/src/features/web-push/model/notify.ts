import { invoke, isDesktop } from '../../../bridge';
import { AGENT_KIND_LABEL, ALLOW, ALWAYS_ALLOW, DENY, feedbackText, isYesNoPermission, offersAlways, sessionStateOf, useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

const FEEDBACK_SETTLE_MS = 400;
const DETAIL_CHARS = 160;

type PushNode = { key: string; id: string };

export function pushToPhones(node: PushNode, kind: string, text: string): void {
  if (!isDesktop) return;
  setTimeout(() => {
    const request = useBerth()?.feedback.value.get(node.id);
    const state = sessionStateOf(node.key);
    const name = state?.name ?? '';
    const agent = state?.kind ? AGENT_KIND_LABEL[state.kind] : name;
    const answerable = kind === 'needs_feedback' && request !== undefined && isYesNoPermission(request);
    const actions = answerable
      ? [
          { action: 'allow', title: t('push.allow'), data: ALLOW, done: t('push.allowed', { name }) },
          ...(offersAlways(request) ? [{ action: 'always', title: t('push.always'), data: ALWAYS_ALLOW, done: t('push.alwaysAllowed', { name }) }] : []),
          { action: 'deny', title: t('push.deny'), data: DENY, done: t('push.answerDenied', { agent }) },
          { action: 'open', title: t('push.open') },
        ]
      : [];
    const asked = request ? [feedbackText(request), request.detail?.slice(0, DETAIL_CHARS)].filter(Boolean).join(': ') : '';
    void invoke('remote_push_notify', {
      payload: { title: 'Berth', body: asked ? `${text}\n${asked}` : text, tag: `berth-${node.key}`, session: { Agent: node.id }, actions, failed: t('push.failed'), openApp: t('push.openApp') },
    }).catch(() => undefined);
  }, FEEDBACK_SETTLE_MS);
}
