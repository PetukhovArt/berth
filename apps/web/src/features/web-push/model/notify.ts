import { invoke, isDesktop } from '../../../bridge';
import { ALLOW, ALWAYS_ALLOW, DENY, feedbackText, isYesNoPermission, offersAlways, useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

const FEEDBACK_SETTLE_MS = 400;
const DETAIL_CHARS = 160;

type PushNode = { key: string; id: string };

export function pushToPhones(node: PushNode, kind: string, text: string): void {
  if (!isDesktop) return;
  setTimeout(() => {
    const request = useBerth()?.feedback.value.get(node.id);
    const answerable = kind === 'waiting' && request !== undefined && isYesNoPermission(request);
    const actions = answerable
      ? [
          { action: 'allow', title: t('push.allow'), data: ALLOW },
          ...(offersAlways(request) ? [{ action: 'always', title: t('push.always'), data: ALWAYS_ALLOW }] : []),
          { action: 'deny', title: t('push.deny'), data: DENY },
          { action: 'open', title: t('push.open') },
        ]
      : [];
    const asked = request ? [feedbackText(request), request.detail?.slice(0, DETAIL_CHARS)].filter(Boolean).join(': ') : '';
    void invoke('remote_push_notify', {
      payload: { title: 'Berth', body: asked ? `${text}\n${asked}` : text, tag: `berth-${node.key}`, session: { Agent: node.id }, actions, failed: t('push.failed') },
    }).catch(() => undefined);
  }, FEEDBACK_SETTLE_MS);
}
