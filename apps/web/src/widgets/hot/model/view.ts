import { computed } from 'vue';
import { useSettings } from '../../../shared/config/settings/store';

const { values } = useSettings();

export const hotEnabled = computed(() => (values.value as Record<string, unknown>)['workbench.hot'] !== 'off');
