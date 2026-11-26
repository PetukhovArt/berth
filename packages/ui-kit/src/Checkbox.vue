<script setup lang="ts">
import { useId } from 'vue';
import Checkbox from 'primevue/checkbox';

defineOptions({ inheritAttrs: false });

const props = withDefaults(defineProps<{ label?: string; binary?: boolean; size?: 'small' | 'large'; inputId?: string }>(), { binary: true, size: 'small', inputId: undefined });
const model = defineModel<unknown>();
const generated = useId();
const id = () => props.inputId ?? generated;
</script>

<template>
  <span class="vr-checkbox">
    <Checkbox v-bind="$attrs" v-model="model" :binary="binary" :size="size" :input-id="id()" />
    <label v-if="label || $slots.default" class="vr-checkbox-label" :for="id()"><slot>{{ label }}</slot></label>
  </span>
</template>
