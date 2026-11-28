<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Check, ChevronDown, ChevronRight, ChevronUp, LoaderCircle } from '../../../shared/ui/icons';
import { VButton, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import type { PlanItem } from '../model/plan';

const props = withDefaults(defineProps<{ plan: readonly PlanItem[]; phases?: readonly PlanItem[]; startOpen?: boolean }>(), { phases: () => [], startOpen: true });

const { t } = useI18n();
const phone = useIsPhone();
const open = shallowRef(props.startOpen && !phone.value);
const done = computed(() => props.plan.filter((item) => item.state === 'done').length);
const current = computed(() => (props.plan.find((item) => item.state === 'doing') ?? props.plan.find((item) => item.state === 'todo'))?.text);
const toggleLabel = computed(() => {
  const count = t('workbench.external.plan', { done: done.value, total: props.plan.length });
  return phone.value && !open.value && current.value ? `${count} · ${current.value}` : count;
});
</script>

<template>
  <div v-if="plan.length || phases.length" class="plan" :data-open="open" data-testid="conversation-plan">
    <div class="inner">
      <div class="head">
        <ol v-if="phases.length" class="phases" :aria-label="t('chat.plan.phases')" data-testid="conversation-plan-phases">
          <template v-for="(phase, index) in phases" :key="index">
            <ChevronRight v-if="index" :size="11" class="sep" />
            <li v-tooltip="phase.text" class="phase" :data-state="phase.state">{{ phase.text }}</li>
          </template>
        </ol>
        <VButton v-if="plan.length" class="plan-toggle" :label="toggleLabel" :icon-pos="phone ? 'left' : 'right'" text severity="secondary" size="small" :aria-expanded="open" data-testid="conversation-plan-toggle" @click="open = !open">
          <template #icon>
            <ChevronUp v-if="open" :size="12" />
            <ChevronDown v-else :size="12" />
          </template>
        </VButton>
      </div>
      <ul v-if="open && plan.length" class="plan-items">
        <li v-for="(item, index) in plan" :key="index" class="plan-item" :data-state="item.state" data-testid="conversation-plan-item">
          <Check v-if="item.state === 'done'" :size="13" class="plan-icon" />
          <LoaderCircle v-else-if="item.state === 'doing'" :size="13" class="plan-icon" />
          <span v-else class="plan-ring" />
          <span class="plan-text">{{ item.text }}</span>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.plan {
  flex: none;
  padding: 10px var(--vr-space-6);
  border-bottom: 1px solid var(--vr-color-border);
}

.inner {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: var(--vr-size-column);
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-3);
  min-width: 0;
}

.phases {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  margin: 0;
  padding: 0;
  overflow: hidden;
  list-style: none;
}

.sep {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.phase {
  flex: 0 1 auto;
  min-width: 0;
  height: 22px;
  padding: 0 10px;
  overflow: hidden;
  font-size: var(--vr-font-size-control);
  line-height: 1.75;
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
  text-overflow: ellipsis;
  border-radius: var(--vr-radius-full);
}

.phase[data-state='done'] {
  color: var(--vr-color-text-muted);
}

.phase[data-state='doing'] {
  font-weight: 600;
  color: var(--vr-status-running);
  background: var(--vr-tint-running);
}

.plan-toggle {
  flex: none;
  margin-left: auto;
}

:root[data-vr-viewport='phone'] .plan {
  padding: var(--vr-space-1) var(--vr-space-3);
}

:root[data-vr-viewport='phone'] .head {
  flex-wrap: wrap;
}

:root[data-vr-viewport='phone'] .plan-toggle {
  flex: 1 1 100%;
  order: -1;
  min-width: 0;
  margin-left: 0;
  justify-content: flex-start;
}

:root[data-vr-viewport='phone'] .plan[data-open='false'] .phases {
  display: none;
}

.plan-items {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--vr-space-1) var(--vr-space-4);
  max-height: 160px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.plan-item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-control);
}

.plan-item[data-state='done'] {
  color: var(--vr-color-text-muted);
  text-decoration: line-through;
}

.plan-icon {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.plan-item[data-state='done'] .plan-icon {
  color: var(--vr-status-finished);
}

.plan-item[data-state='doing'] .plan-icon {
  color: var(--vr-status-running);
}

.plan-ring {
  flex: none;
  width: 11px;
  height: 11px;
  margin: 1px;
  border: 1.5px solid var(--vr-color-text-subtle);
  border-radius: var(--vr-radius-full);
}

.plan-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
