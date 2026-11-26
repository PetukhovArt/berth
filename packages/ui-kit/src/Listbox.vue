<script setup lang="ts">
import { computed, ref } from 'vue';
import Listbox from 'primevue/listbox';
import { FILTER_FROM_OPTIONS, highlight, selectFilterTexts } from './optionFilter';

type Option = { value: string; label: string; disabled?: boolean };

const props = withDefaults(defineProps<{ options: Option[]; label: string; filter?: boolean; multiple?: boolean; allowEmpty?: boolean }>(), { filter: undefined });
const model = defineModel<string | string[] | null>();
const query = ref('');
const remount = ref(0);
const filtered = computed(() => props.filter ?? props.options.length >= FILTER_FROM_OPTIONS);

function onUpdate(value: string | string[] | null) {
  if (value === null && !props.multiple && !props.allowEmpty) {
    remount.value += 1;
    return;
  }
  model.value = value;
}
</script>

<template>
  <Listbox
    :key="remount"
    :model-value="model"
    class="vr-listbox"
    :options="options"
    option-label="label"
    option-value="value"
    option-disabled="disabled"
    :multiple="multiple"
    :meta-key-selection="!multiple"
    :filter="filtered"
    :filter-placeholder="selectFilterTexts.placeholder"
    :aria-label="label"
    scroll-height="16rem"
    @update:model-value="onUpdate"
    @filter="query = $event.value"
  >
    <template #option="{ option }">
      <slot name="option" :option="option">
        <template v-if="filtered && query"><component :is="() => highlight(option.label, query)" /></template>
        <template v-else>{{ option.label }}</template>
      </slot>
    </template>
  </Listbox>
</template>
