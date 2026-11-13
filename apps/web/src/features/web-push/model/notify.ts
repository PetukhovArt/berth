import { invoke, isDesktop } from '../../../bridge';
import { ALLOW, DENY, useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

const FEEDBACK_SETTLE_MS = 400;

type PushNode = { key: string; id: string };

export function pushToPhones(node: PushNode, kind: string, text: string): void {
  if (!isDesktop) return;
  setTimeout(() => {
    const request = useBerth()?.feedback.value.get(node.id);
    const actions =
      kind === 'waiting' && request?.kind === 'Permission'
        ? [
            { action: 'allow', title: t('push.allow'), data: ALLOW },
            { action: 'deny', title: t('push.deny'), data: DENY },
          ]
        : [];
    void invoke('remote_push_notify', {
      payload: { title: 'Berth', body: request?.title ? `${text}: ${request.title}` : text, tag: `berth-${node.key}`, session: { Agent: node.id }, actions, failed: t('push.failed') },
    }).catch(() => undefined);
  }, FEEDBACK_SETTLE_MS);
}
