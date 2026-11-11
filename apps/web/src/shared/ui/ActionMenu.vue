<script setup lang="ts">
import { useTemplateRef } from 'vue';
import { VButton, VPopover } from './kit';
import { t } from '../lib/i18n/i18n';

export type MenuItem = { key: string; label: string; testid: string; disabled?: boolean };

defineProps<{ testid: string; label: string; removeLabel: string; extraLabel?: string; extraTestid?: string; createLabel?: string; items?: readonly MenuItem[] }>();
const emit = defineEmits<{ rename: []; remove: []; extra: []; create: []; item: [key: string] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');

function choose(action: 'create' | 'rename' | 'remove' | 'extra') {
  popover.value?.hide();
  if (action === 'create') emit('create');
  else if (action === 'rename') emit('rename');
  else if (action === 'remove') emit('remove');
  else emit('extra');
}

function pick(key: string) {
  popover.value?.hide();
  emit('item', key);
}
</script>

<template>
  <VButton label="⋯" size="small" text severity="secondary" :aria-label="label" aria-haspopup="menu" :data-testid="testid" @click="popover?.toggle($event)" />
  <VPopover ref="popover">
    <div class="menu" role="menu">
      <button v-if="createLabel" class="item" type="button" role="menuitem" data-testid="new-session" @click="choose('create')">{{ createLabel }}</button>
      <button v-for="item in items" :key="item.key" class="item" type="button" role="menuitem" :disabled="item.disabled" :data-testid="item.testid" @click="pick(item.key)">{{ item.label }}</button>
      <button class="item" type="button" role="menuitem" data-testid="rename" @click="choose('rename')">{{ t('common.rename') }}</button>
      <button v-if="extraLabel" class="item" type="button" role="menuitem" :data-testid="extraTestid" @click="choose('extra')">{{ extraLabel }}</button>
      <button class="item danger" type="button" role="menuitem" data-testid="remove" @click="choose('remove')">{{ removeLabel }}</button>
    </div>
  </VPopover>
</template>

<style scoped>
.menu {
  display: flex;
  flex-direction: column;
  min-width: 160px;
}

.item {
  padding: var(--vr-space-2) var(--vr-space-3);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.item:disabled {
  color: var(--vr-color-text-muted);
}

.item:not(:disabled):hover,
.item:not(:disabled):focus-visible {
  background: var(--vr-color-nav-hover);
}

.danger {
  color: var(--vr-color-danger);
}
</style>
