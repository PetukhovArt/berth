import type { KitLocale } from '../shared/ui/kit';
import { t } from '../shared/lib/i18n/i18n';

export const kitLocale = (): KitLocale => ({
  emptyMessage: t('common.kit.emptyMessage'),
  emptyFilterMessage: t('common.kit.emptySearchMessage'),
  emptySearchMessage: t('common.kit.emptySearchMessage'),
  searchMessage: t('common.kit.searchMessage'),
  selectionMessage: t('common.kit.selectionMessage'),
  emptySelectionMessage: t('common.kit.emptySelectionMessage'),
  aria: { close: t('common.close'), selectAll: t('common.kit.selectAll'), unselectAll: t('common.kit.unselectAll') },
});
