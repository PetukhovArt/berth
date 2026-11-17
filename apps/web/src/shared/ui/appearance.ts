import { computed, shallowRef, watchEffect } from 'vue';
import { DARK_CLASS } from './kit';
import { SETTINGS } from '../config/settings/schema';
import { useSettings } from '../config/settings/store';
import { hostWindow } from '../../bridge';

const { values, set } = useSettings();
const scheme = window.matchMedia('(prefers-color-scheme: dark)');
const systemDark = shallowRef(scheme.matches);
scheme.addEventListener('change', (event) => {
  systemDark.value = event.matches;
});

export const isDark = computed(() => (values.value.theme === 'system' ? systemDark.value : values.value.theme === 'dark'));

const fontScale = computed(() => values.value['ui.font_size'] / SETTINGS['ui.font_size'].default);

export function toggleTheme(): void {
  set('theme', isDark.value ? 'light' : 'dark');
}

export function initAppearance(): () => void {
  const root = document.documentElement;
  const stopTheme = watchEffect(() => {
    root.classList.toggle(DARK_CLASS, isDark.value);
    const [red, green, blue] = getComputedStyle(document.body).backgroundColor.match(/\d+(\.\d+)?/g)?.map(Number) ?? [];
    if (red !== undefined && green !== undefined && blue !== undefined) void hostWindow.setBackground([red, green, blue]).catch(() => undefined);
  });
  const stopScale = watchEffect(() => {
    if (fontScale.value === 1) root.style.removeProperty('--vr-font-scale');
    else root.style.setProperty('--vr-font-scale', String(fontScale.value));
  });
  return () => {
    stopTheme();
    stopScale();
  };
}
