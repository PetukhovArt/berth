<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { VButton, VMenu } from './kit';
import { t } from '../lib/i18n/i18n';

export type MenuItem = { key: string; label: string; testid: string; disabled?: boolean };

const props = defineProps<{ testid: string; label: string; removeLabel: string; extraLabel?: string; extraTestid?: string; createLabel?: string; items?: readonly MenuItem[] }>();
const emit = defineEmits<{ rename: []; remove: []; extra: []; create: []; item: [key: string] }>();

const menu = useTemplateRef<InstanceType<typeof VMenu>>('menu');

const model = computed(() => [
  ...(props.createLabel ? [{ label: props.createLabel, testid: 'new-session', command: () => emit('create') }] : []),
  ...(props.items ?? []).map((item) => ({ label: item.label, testid: item.testid, disabled: item.disabled, command: () => emit('item', item.key) })),
  { label: t('common.rename'), testid: 'rename', command: () => emit('rename') },
  ...(props.extraLabel ? [{ label: props.extraLabel, testid: props.extraTestid, command: () => emit('extra') }] : []),
  { label: props.removeLabel, testid: 'remove', danger: true, command: () => emit('remove') },
]);
</script>

<template>
  <VButton label="⋯" size="small" text severity="secondary" :aria-label="label" aria-haspopup="menu" :data-testid="testid" @click="menu?.toggle($event)" />
  <VMenu ref="menu" :model="model" :label="label" />
</template>
