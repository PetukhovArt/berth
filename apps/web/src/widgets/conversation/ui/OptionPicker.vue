<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { Check, ChevronDown } from '../../../shared/ui/icons';
import { VPopover } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import type { AcpConfigOption } from '../../../entities/session';

const props = defineProps<{ option: AcpConfigOption; kind: 'model' | 'effort' }>();
const emit = defineEmits<{ pick: [id: string, value: string] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const current = computed(() => props.option.choices.find((choice) => choice.value === props.option.current)?.name ?? props.option.current);
const title = computed(() => (props.option.applies === 'next_start' ? `${props.option.name} · ${t('chat.option.applyOnRestart')}` : props.option.name));

function pick(value: string) {
  popover.value?.hide();
  if (value !== props.option.current) emit('pick', props.option.id, value);
}
</script>

<template>
  <UiButton type="button" class="picker" :title="title" aria-haspopup="listbox" :data-testid="`chat-${kind}`" @click="popover?.toggle($event)">
    <span class="name">{{ current }}</span>
    <ChevronDown :size="12" />
  </UiButton>
  <VPopover ref="popover">
    <div class="list" role="listbox" :aria-label="option.name">
      <UiButton v-for="choice in option.choices" :key="choice.value" type="button" class="item" role="option" :aria-selected="choice.value === option.current" :title="choice.description ?? undefined" :data-testid="`chat-${kind}-choice`" @click="pick(choice.value)">
        <span class="check"><Check v-if="choice.value === option.current" :size="13" /></span>
        {{ choice.name }}
      </UiButton>
    </div>
  </VPopover>
</template>

<style scoped>
.picker {
  display: inline-flex;
  flex: 0 1 auto;
  gap: var(--vr-space-1);
  align-items: center;
  min-width: 0;
  max-width: 40%;
  height: 26px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-control);
}

.picker:hover {
  color: var(--vr-color-text);
}

.name {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.list {
  display: flex;
  flex-direction: column;
  min-width: 180px;
  max-height: min(320px, 45vh);
  overflow-y: auto;
}

.item {
  display: flex;
  gap: var(--vr-space-2);
  align-items: center;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text);
  text-align: left;
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.item:hover,
.item:focus-visible {
  background: var(--vr-color-nav-hover);
}

.check {
  display: inline-flex;
  flex: none;
  width: 14px;
}
</style>
