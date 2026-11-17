import { t, useI18n } from '../../../shared/lib/i18n/i18n';

const DAY = 86_400_000;

const dayStart = (ms: number) => {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
};

export function activityLabel(ms: number, now = Date.now()): string {
  const days = Math.round((dayStart(now) - dayStart(ms)) / DAY);
  if (days <= 0) return t('workbench.firstRun.activity.today');
  if (days === 1) return t('workbench.firstRun.activity.yesterday');
  if (days < 14) return t('workbench.firstRun.activity.daysAgo', { count: days });
  if (days < 60) return t('workbench.firstRun.activity.weeksAgo', { count: Math.floor(days / 7) });
  return useI18n().formatDate(ms, { day: 'numeric', month: 'short', year: days > 300 ? 'numeric' : undefined });
}
