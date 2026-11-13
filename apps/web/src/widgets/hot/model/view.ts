import { computed, shallowRef } from 'vue';
import { useSettings } from '../../../shared/config/settings/store';

const { values } = useSettings();

export const hotEnabled = computed(() => (values.value as Record<string, unknown>)['workbench.hot'] !== 'off');

export const hotOverviewOpen = shallowRef(false);

export function toggleHotOverview(): void {
  hotOverviewOpen.value = !hotOverviewOpen.value;
}

