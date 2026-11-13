<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, type Ref } from 'vue';
import { ArrowDown, ArrowUp, Plus, Trash2 } from '../../../shared/ui/icons';
import { VButton, VInputText, VMessage, VSelect, VToggleSwitch } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError } from '../../../entities/session';
import ConditionRows from './ConditionRows.vue';
import { ROUTE_KINDS, isObject, type Json } from '../model/conditionRows';
import { draftJson, loadRouter, newRule, saveRouter, type RouterDraft } from '../model/routerFile';

const draft = ref(null) as Ref<RouterDraft | null>;
const path = shallowRef('');
const saved = shallowRef('');
const error = shallowRef<string | null>(null);
const busy = shallowRef(false);
const justSaved = shallowRef(false);

const dirty = computed(() => (draft.value ? draftJson(draft.value) !== saved.value : false));
const modeOptions = [
  { value: 'suggest', label: t('router.modeSuggest') },
  { value: 'auto', label: t('router.modeAuto') },
];
const kindOptions = ROUTE_KINDS.map((value) => ({ value, label: value }));
type ActionKey = 'next_in_chain' | 'switch_model' | 'continue_in';
const ACTIONS: ActionKey[] = ['next_in_chain', 'switch_model', 'continue_in'];
const actionOptions = ACTIONS.map((value) => ({ value, label: t(`router.action.${value}`) }));

async function run(work: () => Promise<{ draft: RouterDraft; path: string }>) {
  busy.value = true;
  error.value = null;
  try {
    const loaded = await work();
    draft.value = loaded.draft;
    path.value = loaded.path;
    saved.value = draftJson(loaded.draft);
    return true;
  } catch (e) {
    error.value = describeError(e);
    return false;
  } finally {
    busy.value = false;
  }
}

const reload = () => run(loadRouter);

async function save() {
  if (!draft.value) return;
  const current = draft.value;
  if (await run(() => saveRouter(current))) {
    justSaved.value = true;
    setTimeout(() => (justSaved.value = false), 2000);
  }
}

function move<T>(items: T[], index: number, by: number) {
  const to = index + by;
  if (to < 0 || to >= items.length) return;
  const [item] = items.splice(index, 1);
  if (item !== undefined) items.splice(to, 0, item);
}

function actionKey(action: Json): ActionKey | null {
  if (!isObject(action)) return null;
  const keys = Object.keys(action);
  const key = keys[0] as ActionKey;
  return keys.length === 1 && ACTIONS.includes(key) && isObject(action[key]) ? key : null;
}

function actionBody(action: Json): Record<string, Json> {
  const key = actionKey(action);
  const body = key && isObject(action) ? action[key] : null;
  return isObject(body) ? body : {};
}

function setActionKey(then: Json[], index: number, key: ActionKey) {
  then[index] = key === 'next_in_chain' ? { next_in_chain: {} } : key === 'switch_model' ? { switch_model: { model: 'sonnet' } } : { continue_in: { kind: 'codex' } };
}

function setActionField(then: Json[], index: number, field: 'kind' | 'model', value: string) {
  const key = actionKey(then[index] ?? null);
  if (!key) return;
  const body = { ...actionBody(then[index] ?? null) };
  if (value.trim() || field === 'kind') body[field] = value.trim();
  else delete body[field];
  then[index] = { [key]: body };
}

const str = (value: Json | undefined) => (typeof value === 'string' ? value : '');
const cooldownOf = (value: string | undefined) => {
  const n = Number(value);
  return value?.trim() && Number.isInteger(n) && n >= 0 ? n : null;
};

onMounted(() => void reload());
</script>

<template>
  <section class="router" data-testid="router-panel">
    <header class="head">
      <h2>{{ t('router.title') }}</h2>
      <div class="head-actions">
        <span v-if="justSaved" class="muted" data-testid="router-saved">{{ t('router.saved') }}</span>
        <VButton size="small" severity="secondary" text :label="t('router.revert')" :disabled="!dirty || busy" data-testid="router-revert" @click="reload" />
        <VButton size="small" :label="t('router.save')" :disabled="!dirty" :loading="busy" data-testid="router-save" @click="save" />
      </div>
    </header>
    <p class="muted">{{ t('router.hint', { path }) }}</p>
    <VMessage v-if="error" severity="error" size="small" data-testid="router-error">{{ error }}</VMessage>

    <template v-if="draft">
      <label class="field">
        <span>{{ t('router.mode') }}</span>
        <VSelect v-model="draft.mode" :options="modeOptions" option-label="label" option-value="value" size="small" data-testid="router-mode" />
      </label>

      <div class="block" data-testid="router-chain">
        <h3>{{ t('router.chain') }}</h3>
        <p class="muted">{{ t('router.chainHint') }}</p>
        <ol class="chain">
          <li v-for="(entry, index) in draft.fallback" :key="index" class="chain-row" :data-testid="`router-chain-${index}`">
            <span class="muted">{{ index + 1 }}</span>
            <VSelect v-model="entry.kind" :options="kindOptions" option-label="label" option-value="value" size="small" :aria-label="t('router.chain')" data-testid="router-chain-kind" />
            <VInputText v-model="entry.model" size="small" :placeholder="t('router.modelPlaceholder')" :aria-label="t('router.modelPlaceholder')" data-testid="router-chain-model" />
            <VButton size="small" text severity="secondary" :aria-label="t('router.moveUp')" :title="t('router.moveUp')" :disabled="index === 0" @click="move(draft.fallback, index, -1)">
              <template #icon><ArrowUp :size="12" /></template>
            </VButton>
            <VButton size="small" text severity="secondary" :aria-label="t('router.moveDown')" :title="t('router.moveDown')" :disabled="index === draft.fallback.length - 1" @click="move(draft.fallback, index, 1)">
              <template #icon><ArrowDown :size="12" /></template>
            </VButton>
            <VButton size="small" text severity="secondary" :aria-label="t('router.remove')" :title="t('router.remove')" @click="draft.fallback.splice(index, 1)">
              <template #icon><Trash2 :size="12" /></template>
            </VButton>
          </li>
        </ol>
        <VButton size="small" text :label="t('router.chainAdd')" data-testid="router-chain-add" @click="draft.fallback.push({ kind: 'codex', model: '' })">
          <template #icon><Plus :size="12" /></template>
        </VButton>
      </div>

      <h3>{{ t('router.rules') }}</h3>
      <div v-for="(rule, index) in draft.rules" :key="index" class="block rule" :data-testid="`router-rule-${rule.id}`">
        <div class="rule-head">
          <VToggleSwitch v-model="rule.enabled" :input-id="`router-rule-enabled-${index}`" :aria-label="t('router.ruleEnabled')" data-testid="router-rule-enabled" />
          <VInputText v-model="rule.name" size="small" :placeholder="rule.id" :aria-label="t('router.ruleName')" class="grow" data-testid="router-rule-name" />
          <VInputText v-model="rule.id" size="small" :aria-label="t('router.ruleId')" class="id" data-testid="router-rule-id" />
          <VButton size="small" text severity="secondary" :aria-label="t('router.moveUp')" :title="t('router.moveUp')" :disabled="index === 0" @click="move(draft.rules, index, -1)">
            <template #icon><ArrowUp :size="12" /></template>
          </VButton>
          <VButton size="small" text severity="secondary" :aria-label="t('router.remove')" :title="t('router.remove')" data-testid="router-rule-remove" @click="draft.rules.splice(index, 1)">
            <template #icon><Trash2 :size="12" /></template>
          </VButton>
        </div>
        <h4>{{ t('router.when') }}</h4>
        <ConditionRows v-model="rule.conditions" />
        <h4>{{ t('router.actions') }}</h4>
        <div v-for="(action, at) in rule.actions" :key="at" class="action" data-testid="router-action">
          <template v-if="actionKey(action)">
            <VSelect :model-value="actionKey(action)" :options="actionOptions" option-label="label" option-value="value" size="small" data-testid="router-action-type" @update:model-value="setActionKey(rule.actions, at, $event)" />
            <VSelect v-if="actionKey(action) === 'continue_in'" :model-value="str(actionBody(action).kind)" :options="kindOptions" option-label="label" option-value="value" size="small" data-testid="router-action-kind" @update:model-value="setActionField(rule.actions, at, 'kind', $event)" />
            <VInputText v-if="actionKey(action) !== 'next_in_chain'" :model-value="str(actionBody(action).model)" size="small" :placeholder="t('router.modelPlaceholder')" :aria-label="t('router.modelPlaceholder')" data-testid="router-action-model" @update:model-value="setActionField(rule.actions, at, 'model', $event ?? '')" />
          </template>
          <code v-else class="raw" :title="t('router.kept')" data-testid="router-action-opaque">{{ JSON.stringify(action) }}</code>
          <VButton size="small" text severity="secondary" :aria-label="t('router.remove')" :title="t('router.remove')" @click="rule.actions.splice(at, 1)">
            <template #icon><Trash2 :size="12" /></template>
          </VButton>
        </div>
        <div class="rule-foot">
          <VButton size="small" text :label="t('router.addAction')" data-testid="router-add-action" @click="rule.actions.push({ next_in_chain: {} })">
            <template #icon><Plus :size="12" /></template>
          </VButton>
          <label class="field inline">
            <span class="muted">{{ t('router.cooldown') }}</span>
            <VInputText :model-value="rule.cooldownSecs === null ? '' : String(rule.cooldownSecs)" size="small" class="num" placeholder="600" data-testid="router-rule-cooldown" @update:model-value="rule.cooldownSecs = cooldownOf($event)" />
          </label>
        </div>
      </div>
      <VButton size="small" :label="t('router.ruleAdd')" severity="secondary" data-testid="router-rule-add" @click="draft.rules.push(newRule(draft.rules.map((r) => r.id)))">
        <template #icon><Plus :size="12" /></template>
      </VButton>
    </template>
  </section>
</template>

<style scoped>
.router {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: 100%;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.head,
.head-actions,
.rule-head,
.rule-foot,
.chain-row,
.action,
.field.inline {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.head {
  justify-content: space-between;
}

h2 {
  margin: 0;
  font-size: var(--vr-font-size-lg);
}

h3,
h4 {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

p {
  margin: 0;
}

.muted {
  color: var(--vr-color-text-muted);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.chain {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.grow {
  flex: 1;
}

.id {
  width: 9rem;
}

.num {
  width: 6rem;
}

.rule-foot {
  justify-content: space-between;
}

.raw {
  max-width: 28rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
