import { defineComponent, h } from 'vue';
import Dialog from 'primevue/dialog';

const closeButton = { severity: 'secondary', text: true, size: 'small' };

const Wrapped = defineComponent({
  name: 'VDialog',
  inheritAttrs: false,
  setup(_, { attrs, slots }) {
    return () => h(Dialog, { ...attrs, closeButtonProps: { ...closeButton, ...(attrs.closeButtonProps ?? attrs['close-button-props'] as object | undefined) } }, slots);
  },
});

export const VDialog = Wrapped as unknown as typeof Dialog;
