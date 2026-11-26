import { defineComponent, h, mergeProps, nextTick, ref, type PropType, type Slots } from 'vue';
import Select from 'primevue/select';
import { FILTER_FROM_OPTIONS, highlight, labelOf, selectFilterTexts, type LabelSource } from './optionFilter';

type SelectInstance = { focusedOptionIndex: number; findFirstOptionIndex: () => number; scrollInView: () => void };

const Wrapped = defineComponent({
  name: 'VSelect',
  inheritAttrs: false,
  props: {
    options: { type: Array as PropType<unknown[]>, default: undefined },
    editable: Boolean,
    filter: { type: Boolean, default: undefined },
    filterPlaceholder: { type: String, default: undefined },
  },
  setup(props, { attrs, slots }) {
    const select = ref<SelectInstance | null>(null);
    const query = ref('');

    const onFilter = (event: { value: string }) => {
      query.value = event.value;
      if (!event.value) return;
      void nextTick(() => {
        const inst = select.value;
        if (!inst) return;
        inst.focusedOptionIndex = inst.findFirstOptionIndex();
        inst.scrollInView();
      });
    };

    const optionAttr = (camel: string) => attrs[camel] ?? attrs[camel.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)];

    const selectedLabel = (value: unknown): string | undefined => {
      const groupChildren = optionAttr('optionGroupChildren') as string | undefined;
      const flat = (props.options ?? []).flatMap((option) => (groupChildren && option && typeof option === 'object' ? (((option as Record<string, unknown>)[groupChildren] as unknown[] | undefined) ?? []) : [option]));
      const valueKey = optionAttr('optionValue') as string | undefined;
      const found = flat.find((option) => (valueKey && option && typeof option === 'object' ? (option as Record<string, unknown>)[valueKey] : option) === value);
      return found === undefined ? undefined : labelOf(found, optionAttr('optionLabel') as LabelSource);
    };

    return () => {
      const filter = !props.editable && (props.filter ?? (props.options?.length ?? 0) >= FILTER_FROM_OPTIONS);
      const extra: Record<string, (slot: any) => unknown> = {};
      if (filter) {
        if (!slots.option) {
          extra.option = (slot: { option: unknown }) => highlight(labelOf(slot.option, optionAttr('optionLabel') as LabelSource), query.value);
        }
        if (!slots.value) {
          extra.value = (slot: { value: unknown; placeholder?: string }) => selectedLabel(slot.value) ?? slot.placeholder ?? ' ';
        }
        if (!slots.footer) {
          extra.footer = (slot: { options: unknown[] }) =>
            query.value && selectFilterTexts.count
              ? h('div', { class: 'vr-select-count' }, selectFilterTexts.count.replace('{0}', String(slot.options.length)).replace('{1}', String(props.options?.length ?? 0)))
              : null;
        }
      }
      return h(
        Select,
        mergeProps(attrs, {
          ref: select,
          options: props.options,
          editable: props.editable,
          filter,
          autoFilterFocus: true,
          resetFilterOnHide: true,
          filterPlaceholder: props.filterPlaceholder ?? selectFilterTexts.placeholder,
          onFilter,
          onHide: () => {
            query.value = '';
          },
        }),
        { ...slots, ...extra } as unknown as Slots,
      );
    };
  },
});

export const VSelect = Wrapped as unknown as typeof Select;
