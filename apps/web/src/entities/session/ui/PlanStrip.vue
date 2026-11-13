<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Check, ChevronDown, ChevronUp, LoaderCircle } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import type { PlanItem } from '../model/plan';

const props = withDefaults(defineProps<{ plan: readonly PlanItem[]; startOpen?: boolean }>(), { startOpen: true });

const { t } = useI18n();
const open = shallowRef(props.startOpen);
const done = computed(() => props.plan.filter((item) => item.state === 'done').length);
</script>

<template>
  <div v-if="plan.length" class="plan" data-testid="conversation-plan">
    <UiButton type="button" class="plan-toggle" :aria-expanded="open" data-testid="conversation-plan-toggle" @click="open = !open">
      {{ t('workbench.external.plan', { done, total: plan.length }) }}
      <ChevronUp v-if="open" :size="12" />
      <ChevronDown v-else :size="12" />
    </UiButton>
    <ul v-if="open" class="plan-items">
      <li v-for="(item, index) in plan" :key="index" class="plan-item" :data-state="item.state" data-testid="conversation-plan-item">
        <Check v-if="item.state === 'done'" :size="13" class="plan-icon" />
        <LoaderCircle v-else-if="item.state === 'doing'" :size="13" class="plan-icon" />
        <span v-else class="plan-ring" />
        <span class="plan-text">{{ item.text }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.plan {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.plan-toggle {
  display: inline-flex;
  align-items: center;
  align-self: flex-end;
  gap: var(--vr-space-1);
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: none;
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
  font-size: var(--vr-font-size-xs);
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
