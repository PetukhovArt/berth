<script setup lang="ts">
import { useAttrs, useTemplateRef } from 'vue';

const props = withDefaults(defineProps<{ checked?: boolean }>(), { checked: undefined });
const model = defineModel<string | boolean>();
const attrs = useAttrs();
const el = useTemplateRef<HTMLInputElement>('el');

defineExpose({ focus: () => el.value?.focus() });
</script>

<template>
  <input v-if="props.checked !== undefined" ref="el" type="checkbox" :checked="props.checked" />
  <input v-else-if="attrs.type === 'checkbox'" ref="el" v-model="model" type="checkbox" />
  <input v-else ref="el" v-model="model" />
</template>
