import { t } from '../../../shared/lib/i18n/i18n';

const MINUTE = 60;

export function durationLabel(seconds: number): string {
  if (seconds < MINUTE) return t('chat.seconds', { count: seconds });
  return t('chat.minutes', { minutes: Math.floor(seconds / MINUTE), seconds: seconds % MINUTE });
}
