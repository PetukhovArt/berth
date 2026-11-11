<script setup lang="ts">
import { useTemplateRef } from 'vue';

const model = defineModel<string>();
const el = useTemplateRef<HTMLTextAreaElement>('el');

defineExpose({ focus: () => el.value?.focus() });
</script>

<template>
  <textarea ref="el" v-model="model" />
</template>
