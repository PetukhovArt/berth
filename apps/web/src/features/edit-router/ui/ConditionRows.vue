<script setup lang="ts">
import { Plus, Trash2 } from '../../../shared/ui/icons';
import { VButton, VInputText, VSelect } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { ERROR_KINDS, ROUTE_KINDS, STATUSES, closeOf, conditionRow, depthOf, newCondition, groupRows, insertRows, removeRows, type Condition, type ConditionList, type ConditionType, type GroupOp, type Row } from '../model/conditionRows';

const list = defineModel<ConditionList>({ required: true });

const TYPES: ConditionType[] = ['provider_error', 'limit_used', 'status', 'kind', 'model'];
const typeOptions = TYPES.map((value) => ({ value, label: t(`router.condition.${value}`) }));
const joinOptions = (['all', 'any'] as GroupOp[]).map((value) => ({ value, label: t(`router.join.${value}`) }));

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

function toggle(values: string[], value: string): string[] {
  return values.includes(value) ? values.filter((v) => v !== value) : [...values, value];
}

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
        <VSelect :model-value="row.op" :options="joinOptions" option-label="label" option-value="value" size="small" class="join" :aria-label="t('router.join.all')" data-testid="router-join" @update:model-value="patch(index, { ...row, op: $event })" />
      </template>
      <template v-else-if="row.type === 'close'">
        <span class="bracket">)</span>
        <VButton size="small" text severity="secondary" :label="t('router.addCondition')" data-testid="router-group-add" @click="addAt(index, [conditionRow('provider_error')])">
          <template #icon><Plus :size="12" /></template>
        </VButton>
      </template>
      <template v-else>
        <UiButton type="button" class="not" :aria-pressed="row.negate" data-testid="router-not" @click="patch(index, { ...row, negate: !row.negate })">{{ t('router.not') }}</UiButton>
        <template v-if="row.type === 'open'">
          <span class="bracket">( {{ t('router.group') }}</span>
        </template>
        <template v-else-if="row.type === 'opaque'">
          <code class="raw" :title="t('router.kept')" data-testid="router-opaque">{{ JSON.stringify(row.raw) }}</code>
        </template>
        <template v-else>
          <VSelect :model-value="row.condition.type" :options="typeOptions" option-label="label" option-value="value" size="small" class="type" data-testid="router-condition-type" @update:model-value="changeType(index, row, $event)" />
          <template v-if="row.condition.type === 'provider_error'">
            <span class="chips">
              <UiButton v-for="kind in ERROR_KINDS" :key="kind" type="button" class="chip" :aria-pressed="row.condition.kinds.includes(kind)" :data-testid="`router-error-${kind}`" @click="setCondition(index, row, { ...row.condition, kinds: toggle(row.condition.kinds, kind) })">{{ t(`agents.route.errorKind.${kind}`) }}</UiButton>
            </span>
            <VInputText :model-value="String(row.condition.forSecs)" size="small" class="num" :aria-label="t('router.forSecs')" data-testid="router-for-secs" @update:model-value="setCondition(index, row, { ...row.condition, forSecs: num($event, 0) })" />
            <span class="muted">{{ t('router.forSecs') }}</span>
          </template>
          <template v-else-if="row.condition.type === 'limit_used'">
            <VInputText :model-value="String(row.condition.pct)" size="small" class="num" :aria-label="t('router.pct')" data-testid="router-pct" @update:model-value="setCondition(index, row, { ...row.condition, pct: num($event, 95) })" />
            <span class="muted">{{ t('router.pct') }}</span>
            <VInputText :model-value="row.condition.windowMinutes === null ? '' : String(row.condition.windowMinutes)" size="small" class="num" :placeholder="t('router.anyWindow')" :aria-label="t('router.windowMinutes')" data-testid="router-window" @update:model-value="setCondition(index, row, { ...row.condition, windowMinutes: $event?.trim() ? num($event, 0) : null })" />
            <span class="muted">{{ t('router.windowMinutes') }}</span>
          </template>
          <template v-else-if="row.condition.type === 'model'">
            <VInputText :model-value="row.condition.values.join(', ')" size="small" :placeholder="t('router.modelsPlaceholder')" :aria-label="t('router.condition.model')" data-testid="router-models" @update:model-value="setCondition(index, row, { ...row.condition, values: models($event) })" />
          </template>
          <span v-else class="chips">
            <UiButton v-for="value in row.condition.type === 'status' ? STATUSES : ROUTE_KINDS" :key="value" type="button" class="chip" :aria-pressed="row.condition.values.includes(value)" :data-testid="`router-value-${value}`" @click="setCondition(index, row, { ...row.condition, values: toggle(row.condition.values, value) })">{{ value }}</UiButton>
          </span>
        </template>
        <VButton size="small" text severity="secondary" :aria-label="t('router.remove')" :title="t('router.remove')" data-testid="router-row-remove" @click="remove(index)">
          <template #icon><Trash2 :size="12" /></template>
        </VButton>
      </template>
    </div>
    <div class="add">
      <VButton size="small" text :label="t('router.addCondition')" data-testid="router-add-condition" @click="addAt(list.rows.length, [conditionRow('provider_error')])">
        <template #icon><Plus :size="12" /></template>
      </VButton>
      <VButton size="small" text severity="secondary" :label="t('router.addGroup')" data-testid="router-add-group" @click="addAt(list.rows.length, groupRows())" />
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

.chip,
.not {
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text-muted);
  background: transparent;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  cursor: pointer;
}

.chip[aria-pressed='true'],
.not[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border-color: var(--vr-color-text-muted);
}
</style>
