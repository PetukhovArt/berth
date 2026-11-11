import { h, type FunctionalComponent } from 'vue';

export const UiButton: FunctionalComponent = (_props, { slots, attrs }) => h('button', attrs, slots.default?.());

UiButton.inheritAttrs = false;
