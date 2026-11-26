import { watchEffect, type App } from 'vue';
import PrimeVue, { usePrimeVue, type PrimeVueLocaleAriaOptions, type PrimeVueLocaleOptions } from 'primevue/config';
import Aura from '@primeuix/themes/aura';
import { definePreset } from '@primeuix/themes';
import { inkScale } from '@ui/tokens';
import '@fontsource-variable/wix-madefor-text';
import '@fontsource/jetbrains-mono/400.css';
import './fonts.css';
import '@ui/tokens/light.css';
import '@ui/tokens/dark.css';
import '@ui/tokens/motion.css';
import './overlays.css';
import './responsive.css';
import './button.css';
import './components.css';
import { installMotion } from './motion';
import { installViewport } from './viewport';
import { selectFilterTexts } from './optionFilter';

export const DARK_CLASS = 'vr-dark';

const v = (name: string) => `var(--vr-${name})`;

const overlay = { background: v('color-surface-raised'), borderColor: v('color-border'), color: v('color-text'), shadow: v('shadow-popover') };

const scheme = {
  surface: inkScale,
  primary: { color: v('color-accent'), contrastColor: v('color-accent-contrast'), hoverColor: v('color-accent'), activeColor: v('color-accent') },
  highlight: { background: v('color-surface-muted'), focusBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text') },
  formField: {
    background: v('color-surface'),
    disabledBackground: v('color-surface-muted'),
    filledBackground: v('color-surface-muted'),
    filledFocusBackground: v('color-surface'),
    borderColor: v('color-border'),
    hoverBorderColor: v('color-text-muted'),
    focusBorderColor: v('color-accent'),
    color: v('color-text'),
    placeholderColor: v('color-text-muted'),
    floatLabelColor: v('color-text-muted'),
    iconColor: v('color-text-muted'),
  },
  text: { color: v('color-text'), hoverColor: v('color-text'), mutedColor: v('color-text-muted'), hoverMutedColor: v('color-text') },
  content: { background: v('color-surface'), hoverBackground: v('color-surface-muted'), borderColor: v('color-border'), color: v('color-text'), hoverColor: v('color-text') },
  overlay: { select: overlay, popover: overlay, modal: overlay },
  list: { option: { focusBackground: v('color-surface-muted'), selectedBackground: v('color-surface-muted'), selectedFocusBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text'), selectedColor: v('color-text'), selectedFocusColor: v('color-text') } },
  navigation: { item: { focusBackground: v('color-surface-muted'), activeBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text'), activeColor: v('color-text') } },
};

const dangerButton = {
  background: v('btn-danger'),
  hoverBackground: `color-mix(in srgb, ${v('btn-danger')} 88%, transparent)`,
  activeBackground: `color-mix(in srgb, ${v('btn-danger')} 76%, transparent)`,
  borderColor: v('btn-danger'),
  hoverBorderColor: v('btn-danger'),
  activeBorderColor: v('btn-danger'),
  color: v('btn-danger-text'),
  hoverColor: v('btn-danger-text'),
  activeColor: v('btn-danger-text'),
  focusRing: { color: v('btn-danger'), shadow: 'none' },
};

const linkButton = { color: v('color-link'), hoverColor: v('color-link'), activeColor: v('color-link') };

const tint = (token: string, share: number) => `color-mix(in srgb, ${v(token)} ${share}%, white)`;
const shade = (token: string, share: number) => `color-mix(in srgb, ${v(token)} ${share}%, black)`;

const palette = (token: string) => ({
  50: tint(token, 8),
  100: tint(token, 16),
  200: tint(token, 30),
  300: tint(token, 55),
  400: v(token),
  500: v(token),
  600: shade(token, 85),
  700: shade(token, 70),
  800: shade(token, 55),
  900: shade(token, 40),
  950: shade(token, 25),
});

const success = palette('color-success');
const danger = palette('color-danger');
const warn = palette('status-feedback');
const info = palette('status-running');
const help = palette('status-unseen');

const BerthPreset = definePreset(Aura, {
  primitive: {
    borderRadius: { none: '0', xs: '4px', sm: v('radius-sm'), md: v('radius-md'), lg: v('radius-lg'), xl: v('radius-xl') },
    green: success,
    emerald: success,
    red: danger,
    orange: warn,
    yellow: warn,
    amber: warn,
    sky: info,
    blue: info,
    purple: help,
  },
  semantic: {
    transitionDuration: v('motion-base'),
    primary: inkScale,
    focusRing: { color: v('color-accent') },
    colorScheme: { light: scheme, dark: scheme },
  },
  components: {
    toggleswitch: { root: { slideDuration: v('motion-base') } },
    tag: { root: { fontSize: v('font-size-xs'), fontWeight: '500', padding: '1px 8px' } },
    togglebutton: { root: { sm: { fontSize: v('font-size-label'), padding: '2px' } }, content: { sm: { padding: '2px 0.75rem' } } },
    tooltip: {
      root: { maxWidth: '20rem', padding: '4px 8px', borderRadius: v('radius-md'), shadow: v('shadow-popover') },
      colorScheme: { light: { root: { background: v('color-surface-raised'), color: v('color-text') } }, dark: { root: { background: v('color-surface-raised'), color: v('color-text') } } },
    },
    tabs: {
      tablist: { background: 'transparent', borderColor: v('color-border') },
      tab: {
        background: 'transparent',
        hoverBackground: 'transparent',
        activeBackground: 'transparent',
        borderColor: 'transparent',
        hoverBorderColor: 'transparent',
        activeBorderColor: v('color-text'),
        color: v('color-text-muted'),
        hoverColor: v('color-text'),
        activeColor: v('color-text'),
        padding: '0 0 6px 0',
        fontWeight: '400',
        gap: v('space-2'),
      },
      tabpanel: { background: 'transparent', color: v('color-text'), padding: '0' },
      activeBar: { height: '1.5px', bottom: '-1px', background: v('color-text') },
    },
    progressbar: { root: { height: '6px', background: v('color-surface-muted') }, value: { background: v('color-text') } },
    badge: { root: { fontSize: v('font-size-2xs'), fontWeight: '600', minWidth: '1.5em', height: '1.5em', padding: '0 0.4em' } },
    checkbox: { root: { checkedHoverBackground: v('color-accent'), checkedHoverBorderColor: v('color-accent') } },
    button: {
      root: { borderRadius: v('radius-md'), gap: '6px', sm: { fontSize: v('font-size-label'), paddingX: '11px', paddingY: '3px', iconOnlyWidth: '28px' } },
      colorScheme: {
        light: { root: { danger: dangerButton }, outlined: { danger: { color: '{red.600}' } }, text: { danger: { color: '{red.600}' } }, link: linkButton },
        dark: { root: { danger: dangerButton }, link: linkButton },
      },
    },
  },
});

export type KitLocale = Partial<Omit<PrimeVueLocaleOptions, 'aria'>> & { aria?: Partial<PrimeVueLocaleAriaOptions>; filterPlaceholder?: string; filterCount?: string };

export function installUiKit(app: App, locale: () => KitLocale): void {
  app.use(PrimeVue, { theme: { preset: BerthPreset, options: { darkModeSelector: `.${DARK_CLASS}` } } });
  const { config } = app.runWithContext(usePrimeVue);
  const defaults = config.locale;
  if (defaults)
    watchEffect(() => {
      const { aria, filterPlaceholder, filterCount, ...strings } = locale();
      selectFilterTexts.placeholder = filterPlaceholder ?? '';
      selectFilterTexts.count = filterCount ?? '';
      config.locale = { ...defaults, ...strings, aria: { ...defaults.aria, ...aria } };
    });
  installMotion(app);
  installViewport();
}

export { setMotionEnabled, isMotionEnabled } from './motion';
export { useIsPhone } from './viewport';

export { VButton } from './Button';
export { default as VInputText } from 'primevue/inputtext';
export { default as VTextarea } from 'primevue/textarea';
export { VSelect } from './Select';
export { default as VTag } from 'primevue/tag';
export { default as VMessage } from 'primevue/message';
export { default as VMultiSelect } from 'primevue/multiselect';
export { default as VToggleSwitch } from 'primevue/toggleswitch';
export { default as VDialog } from 'primevue/dialog';
export { default as VContextMenu } from 'primevue/contextmenu';
export { default as VPopover } from 'primevue/popover';
export { default as VChipGroup } from './ChipGroup.vue';
export { default as VToggleButton } from 'primevue/togglebutton';
export { default as VBadge } from './Badge.vue';
export { default as VCheckbox } from './Checkbox.vue';
export { default as VProgressBar } from './ProgressBar.vue';
export { default as VTabs } from './Tabs.vue';
export { default as VSelectButton } from './SelectButton.vue';
export { default as VMenu } from './Menu.vue';
export { default as VListbox } from './Listbox.vue';
export { vTooltip } from './Tooltip';
