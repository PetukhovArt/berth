<script setup lang="ts">
import { useAttrs, useTemplateRef } from 'vue';

const model = defineModel<string | boolean>();
const attrs = useAttrs();
const el = useTemplateRef<HTMLInputElement>('el');

defineExpose({ focus: () => el.value?.focus() });
</script>

<template>
  <input v-if="attrs.type === 'checkbox'" ref="el" v-model="model" type="checkbox" />
  <input v-else ref="el" v-model="model" />
</template>
