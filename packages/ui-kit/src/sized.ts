import { defineComponent, h, mergeProps, type Component } from 'vue';

export function withControlSize<T extends Component>(component: T, name: string): T {
  const Wrapped = defineComponent({
    name,
    inheritAttrs: false,
    setup(_, { attrs, slots }) {
      return () => h(component, mergeProps({ size: 'small' }, attrs), slots);
    },
  });
  return Wrapped as unknown as T;
}
