import { computed, shallowRef, watch } from 'vue';
import { treeTab } from '../../../features/work-tree';
import { useDock } from '../../../shared/model/dock';
import { navigate, view } from '../../../shared/model/nav';

export type PhoneSection = 'inbox' | 'sessions' | 'reviews' | 'settings';
type PhoneList = 'inbox' | 'tree';

const STORE_KEY = 'berth.phone.list';

function readList(): PhoneList {
  try {
    return localStorage.getItem(STORE_KEY) === 'tree' ? 'tree' : 'inbox';
  } catch {
    return 'inbox';
  }
}

const list = shallowRef<PhoneList>(readList());

watch(list, (value) => {
  try {
    localStorage.setItem(STORE_KEY, value);
  } catch {
    return;
  }
});

export const phoneSection = computed<PhoneSection | null>(() => {
  if (view.value.kind === 'settings') return 'settings';
  if (view.value.kind !== 'agents') return null;
  if (list.value === 'inbox') return 'inbox';
  return treeTab.value === 'reviews' ? 'reviews' : 'sessions';
});

export function openPhoneSection(section: PhoneSection): void {
  if (section === 'settings') {
    if (view.value.kind !== 'settings') void navigate({ kind: 'settings' });
    return;
  }
  useDock().detail.value = false;
  list.value = section === 'inbox' ? 'inbox' : 'tree';
  if (section === 'reviews') treeTab.value = 'reviews';
  else if (section === 'sessions' && treeTab.value === 'reviews') treeTab.value = 'sessions';
  if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
}
