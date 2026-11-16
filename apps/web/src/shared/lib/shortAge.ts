import { t } from './i18n/i18n';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function shortAge(now: number, since: number): string {
  const diff = Math.max(0, now - since);
  if (diff < MINUTE) return t('attention.inbox.now');
  if (diff < HOUR) return t('attention.inbox.minutes', { count: Math.floor(diff / MINUTE) });
  if (diff < DAY) return t('attention.inbox.hours', { count: Math.floor(diff / HOUR) });
  return t('attention.inbox.days', { count: Math.floor(diff / DAY) });
}
