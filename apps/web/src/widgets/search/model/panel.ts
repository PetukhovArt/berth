import { shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { titleKey, useDock, type PanelSpec } from '../../../shared/model/dock';
import type { SearchScope } from './rows';

const SEARCH_PANEL = 'search';

export const searchQuery = shallowRef('');
export const searchScope = shallowRef<SearchScope>('all');
export const searchFocus = shallowRef(0);

const searchPanel = (): PanelSpec => ({ id: SEARCH_PANEL, component: 'search', title: t('search.title'), params: titleKey('search.title') });

export const reviveSearch = (id: string): PanelSpec | undefined => (id === SEARCH_PANEL ? searchPanel() : undefined);

export function openSearch(query?: string): void {
  if (query !== undefined) {
    searchQuery.value = query;
    searchScope.value = 'all';
  }
  useDock().openPanel(searchPanel());
  searchFocus.value++;
}
