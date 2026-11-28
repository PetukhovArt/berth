<script setup lang="ts">
import { computed } from 'vue';
import { Plus, Trash2 } from '../../../shared/ui/icons';
import { VButton, VInputText, VSelect, VSelectButton, VToggleButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { ERROR_KINDS, ROUTE_KINDS, STATUSES, closeOf, conditionRow, depthOf, newCondition, groupRows, insertRows, removeRows, type Condition, type ConditionList, type ConditionType, type GroupOp, type Row } from '../model/conditionRows';

const list = defineModel<ConditionList>({ required: true });

const TYPES: ConditionType[] = ['provider_error', 'limit_used', 'status', 'kind', 'model'];
const typeOptions = computed(() => TYPES.map((value) => ({ value, label: t(`router.condition.${value}`) })));
const joinOptions = computed(() => (['all', 'any'] as GroupOp[]).map((value) => ({ value, label: t(`router.join.${value}`) })));

function setRows(rows: Row[]) {
  list.value = { ...list.value, rows };
}

function patch(index: number, row: Row) {
  setRows(list.value.rows.map((r, i) => (i === index ? row : r)));
}

function setCondition(index: number, row: Row, condition: Condition) {
  if (row.type === 'condition') patch(index, { ...row, condition });
}

function changeType(index: number, row: Row, type: ConditionType) {
  if (row.type === 'condition' && row.condition.type !== type) patch(index, { id: row.id, type: 'condition', negate: row.negate, condition: newCondition(type) });
}

const errorKindOptions = computed(() => ERROR_KINDS.map((kind) => ({ value: kind, label: t(`agents.route.errorKind.${kind}`), testId: `router-error-${kind}` })));
const statusOptions = STATUSES.map((value) => ({ value, label: value, testId: `router-value-${value}` }));
const routeKindOptions = ROUTE_KINDS.map((value) => ({ value, label: value, testId: `router-value-${value}` }));

function addAt(at: number, rows: Row[]) {
  setRows(insertRows(list.value.rows, at, rows));
}

function remove(index: number) {
  const row = list.value.rows[index];
  setRows(removeRows(list.value.rows, index, row?.type === 'open' ? closeOf(list.value.rows, index) : index));
}

const num = (value: string | undefined, fallback: number) => {
  const n = Number(value);
  return value !== undefined && value.trim() !== '' && Number.isFinite(n) && n >= 0 ? n : fallback;
};
const models = (value: string | undefined) =>
  (value ?? '')
    .split(',')
    .map((m) => m.trim())
    .filter(Boolean);
</script>

<template>
  <div class="rows" data-testid="router-conditions">
    <p v-if="!list.rows.length" class="muted">{{ t('router.empty') }}</p>
    <div v-for="(row, index) in list.rows" :key="row.id" class="row" :style="{ paddingInlineStart: `calc(${depthOf(list.rows, index)} * var(--vr-space-4))` }" :data-row="row.type">
      <template v-if="row.type === 'join'">
        <VSelect :model-value="row.op" :options="joinOptions" option-label="label" option-value="value" class="join" :aria-label="t('router.join.all')" data-testid="router-join" @update:model-value="patch(index, { ...row, op: $event })" />
      </template>
      <template v-else-if="row.type === 'close'">
        <span class="bracket">)</span>
        <VButton size="small" severity="secondary" outlined :label="t('router.addCondition')" data-testid="router-group-add" @click="addAt(index, [conditionRow('provider_error')])">
          <template #icon><Plus :size="14" /></template>
        </VButton>
      </template>
      <template v-else>
        <VToggleButton :model-value="row.negate" :on-label="t('router.not')" :off-label="t('router.not')" size="small" data-testid="router-not" @update:model-value="patch(index, { ...row, negate: $event })" />
        <template v-if="row.type === 'open'">
          <span class="bracket">( {{ t('router.group') }}</span>
        </template>
        <template v-else-if="row.type === 'opaque'">
          <code v-tooltip="t('router.kept')" class="raw" data-testid="router-opaque">{{ JSON.stringify(row.raw) }}</code>
        </template>
        <template v-else>
          <VSelect :model-value="row.condition.type" :options="typeOptions" option-label="label" option-value="value" class="type" data-testid="router-condition-type" @update:model-value="changeType(index, row, $event)" />
          <template v-if="row.condition.type === 'provider_error'">
            <VSelectButton class="chips" :model-value="row.condition.kinds" :options="errorKindOptions" :label="t('router.condition.provider_error')" multiple @update:model-value="setCondition(index, row, { ...row.condition, kinds: $event as string[] })" />
            <VInputText :model-value="String(row.condition.forSecs)" class="num" :aria-label="t('router.forSecs')" data-testid="router-for-secs" @update:model-value="setCondition(index, row, { ...row.condition, forSecs: num($event, 0) })" />
            <span class="muted">{{ t('router.forSecs') }}</span>
          </template>
          <template v-else-if="row.condition.type === 'limit_used'">
            <VInputText :model-value="String(row.condition.pct)" class="num" :aria-label="t('router.pct')" data-testid="router-pct" @update:model-value="setCondition(index, row, { ...row.condition, pct: num($event, 95) })" />
            <span class="muted">{{ t('router.pct') }}</span>
            <VInputText :model-value="row.condition.windowMinutes === null ? '' : String(row.condition.windowMinutes)" class="num" :placeholder="t('router.anyWindow')" :aria-label="t('router.windowMinutes')" data-testid="router-window" @update:model-value="setCondition(index, row, { ...row.condition, windowMinutes: $event?.trim() ? num($event, 0) : null })" />
            <span class="muted">{{ t('router.windowMinutes') }}</span>
          </template>
          <template v-else-if="row.condition.type === 'model'">
            <VInputText :model-value="row.condition.values.join(', ')" :placeholder="t('router.modelsPlaceholder')" :aria-label="t('router.condition.model')" data-testid="router-models" @update:model-value="setCondition(index, row, { ...row.condition, values: models($event) })" />
          </template>
          <VSelectButton
            v-else
            class="chips"
            :model-value="row.condition.values"
            :options="row.condition.type === 'status' ? statusOptions : routeKindOptions"
            :label="t(`router.condition.${row.condition.type}`)"
            multiple
            @update:model-value="setCondition(index, row, { ...row.condition, values: $event as string[] })"
          />
        </template>
        <VButton v-tooltip="t('router.remove')" size="small" text severity="secondary" :aria-label="t('router.remove')" data-testid="router-row-remove" @click="remove(index)">
          <template #icon><Trash2 :size="14" /></template>
        </VButton>
      </template>
    </div>
    <div class="add">
      <VButton size="small" severity="secondary" outlined :label="t('router.addCondition')" data-testid="router-add-condition" @click="addAt(list.rows.length, [conditionRow('provider_error')])">
        <template #icon><Plus :size="14" /></template>
      </VButton>
      <VButton size="small" severity="secondary" outlined :label="t('router.addGroup')" data-testid="router-add-group" @click="addAt(list.rows.length, groupRows())">
        <template #icon><Plus :size="14" /></template>
      </VButton>
    </div>
  </div>
</template>

<style scoped>
.rows {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  min-height: var(--vr-size-row);
}

.join,
.type {
  min-width: 8rem;
}

.num {
  width: 5rem;
}

.bracket,
.muted {
  color: var(--vr-color-text-muted);
}

.raw {
  max-width: 28rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chips,
.add {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
}
</style>
