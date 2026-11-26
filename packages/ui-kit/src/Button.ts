import { defineComponent, h, type Slot } from 'vue';
import Button from 'primevue/button';

const Wrapped = defineComponent({
  name: 'VButton',
  inheritAttrs: false,
  setup(_, { attrs, slots }) {
    return () => {
      const forward: Record<string, Slot | undefined> = { ...slots };
      const icon = slots.icon;
      if (icon && !slots.default) forward.icon = (slot) => [h('span', { class: (slot as { class?: unknown }).class, style: { display: 'inline-flex' } }, icon(slot))];
      return h(Button, attrs, forward);
    };
  },
});

export const VButton = Wrapped as unknown as typeof Button;
