<script setup lang="ts">
import { computed, useAttrs, useTemplateRef } from 'vue';
import { VCheckbox, VInputText } from './kit';

defineOptions({ inheritAttrs: false });

const props = withDefaults(defineProps<{ checked?: boolean }>(), { checked: undefined });
const model = defineModel<string | boolean>();
const attrs = useAttrs();
const el = useTemplateRef<{ $el: HTMLElement }>('el');

const isCheckbox = computed(() => props.checked !== undefined || attrs.type === 'checkbox');
const boxAttrs = computed(() => {
  const { class: _class, type: _type, ...rest } = attrs;
  return rest;
});
const text = computed(() => (typeof model.value === 'string' ? model.value : undefined));
const boxValue = computed(() => (props.checked !== undefined ? props.checked : model.value === true));

function focus() {
  const root = el.value?.$el;
  (root instanceof HTMLInputElement ? root : root?.querySelector('input'))?.focus();
}

defineExpose({ focus });
</script>

<template>
  <VCheckbox v-if="props.checked !== undefined" ref="el" v-bind="boxAttrs" :model-value="boxValue" />
  <VCheckbox v-else-if="isCheckbox" ref="el" v-bind="boxAttrs" v-model="model" />
  <VInputText v-else ref="el" v-bind="attrs" :model-value="text" @update:model-value="model = $event" />
</template>
