<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { ChevronDown } from '../../../shared/ui/icons';
import { VButton, VListbox, VPopover, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import type { AcpConfigOption } from '../../../entities/session';

const props = defineProps<{ option: AcpConfigOption; kind: 'model' | 'effort' }>();
const emit = defineEmits<{ pick: [id: string, value: string] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const current = computed(() => props.option.choices.find((choice) => choice.value === props.option.current)?.name ?? props.option.current);
const title = computed(() => (props.option.applies === 'next_start' ? `${props.option.name} · ${t('chat.option.applyOnRestart')}` : props.option.name));

const choices = computed(() => props.option.choices.map((choice) => ({ value: choice.value, label: choice.name, description: choice.description })));

function pick(value: unknown) {
  popover.value?.hide();
  if (typeof value === 'string' && value !== props.option.current) emit('pick', props.option.id, value);
}
</script>

<template>
  <VButton v-tooltip="title" class="picker" :label="current" icon-pos="right" text severity="secondary" size="small" aria-haspopup="listbox" :data-testid="`chat-${kind}`" @click="popover?.toggle($event)">
    <template #icon><ChevronDown :size="12" /></template>
  </VButton>
  <VPopover ref="popover">
    <div class="list">
      <VListbox flat :model-value="option.current" :options="choices" :label="option.name" :test-id="`chat-${kind}-choice`" @update:model-value="pick">
        <template #option="{ option: choice }">
          <span v-tooltip="choice.description ?? undefined">{{ choice.label }}</span>
        </template>
      </VListbox>
    </div>
  </VPopover>
</template>

<style scoped>
.picker {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 40%;
  height: 26px;
  padding: 0 var(--vr-space-2);
  border-radius: var(--vr-radius-control);
}

.list {
  min-width: 180px;
}
</style>
