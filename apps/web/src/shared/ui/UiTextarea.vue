<script setup lang="ts">
import { useTemplateRef } from 'vue';
import { VTextarea } from './kit';

const model = defineModel<string>();
const el = useTemplateRef<{ $el: HTMLTextAreaElement }>('el');

defineExpose({ focus: () => el.value?.$el.focus() });
</script>

<template>
  <VTextarea ref="el" v-model="model" />
</template>
