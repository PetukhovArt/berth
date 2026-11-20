<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Plus } from '../../../shared/ui/icons';
import { VButton, VInputText } from '../../../shared/ui/kit';
import UiInput from '../../../shared/ui/UiInput.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { permissionHint, permissionLabel, type HubRole } from '../../../entities/session';
import { useChanges } from '../model/changes';

const props = defineProps<{ roles: HubRole[]; catalog: string[] }>();

const GROUPS = ['channels', 'agents', 'context', 'admin'] as const;
const FORM = 'form';

const { busy, errors, run } = useChanges();
const form = shallowRef<{ id: string | null } | null>(null);
const name = shallowRef('');
const selected = shallowRef<ReadonlySet<string>>(new Set());

const groups = computed(() => {
  const byGroup = new Map<string, string[]>();
  for (const key of props.catalog) {
    const group = key.split('.')[0] ?? key;
    byGroup.set(group, [...(byGroup.get(group) ?? []), key]);
  }
  return [...byGroup].map(([id, keys]) => ({ id, label: (GROUPS as readonly string[]).includes(id) ? t(`hub.group.${id as (typeof GROUPS)[number]}`) : id, keys }));
});

const saving = computed(() => busy.value.has(FORM));
const formTitle = computed(() => (form.value?.id ? t('hub.roles.editing', { name: props.roles.find((role) => role.id === form.value?.id)?.name ?? '' }) : t('hub.roles.creating')));

function startNew() {
  form.value = { id: null };
  name.value = '';
  selected.value = new Set();
}

function startEdit(role: HubRole) {
  form.value = { id: role.id };
  name.value = role.name;
  selected.value = new Set(role.permissions);
}

function toggle(key: string, on: boolean) {
  const next = new Set(selected.value);
  if (on) next.add(key);
  else next.delete(key);
  selected.value = next;
}

async function save() {
  const current = form.value;
  const label = name.value.trim();
  if (!current || !label || saving.value) return;
  const permissions = props.catalog.filter((key) => selected.value.has(key));
  if (await run(FORM, { SaveRole: { id: current.id, name: label, permissions } })) form.value = null;
}

const remove = (role: HubRole) => run(role.id, { DeleteRole: { id: role.id } });
</script>

<template>
  <section class="roles" data-testid="hub-admin-roles">
    <h3 class="head">
      {{ t('hub.roles.title') }}
      <VButton :label="t('hub.roles.new')" size="small" severity="secondary" text data-testid="hub-admin-role-new" @click="startNew">
        <template #icon><Plus :size="14" /></template>
      </VButton>
    </h3>
    <p v-if="!roles.length" class="muted" data-testid="hub-admin-roles-empty">{{ t('hub.roles.empty') }}</p>
    <ul v-else class="list" data-testid="hub-admin-roles-list">
      <li v-for="role in roles" :key="role.id" class="role" :data-builtin="role.builtin" :data-testid="`hub-admin-role-${role.id}`">
        <div class="line">
          <span class="name" :title="role.name">{{ role.name }}</span>
          <span v-if="role.builtin" class="chip">{{ t('hub.roles.builtin') }}</span>
          <span class="muted">{{ t('hub.roles.heldBy', { count: role.members }) }}</span>
          <template v-if="!role.builtin">
            <VButton :label="t('hub.roles.edit')" :aria-label="t('hub.roles.editLabel', { name: role.name })" size="small" severity="secondary" text :disabled="busy.has(role.id)" :data-testid="`hub-admin-role-edit-${role.id}`" @click="startEdit(role)" />
            <VButton
              :label="t('hub.roles.delete')"
              :aria-label="t('hub.roles.deleteLabel', { name: role.name })"
              :title="role.members > 0 ? t('hub.roles.deleteHeld', { count: role.members }) : undefined"
              size="small"
              severity="danger"
              text
              :disabled="role.members > 0 || busy.has(role.id)"
              :data-testid="`hub-admin-role-delete-${role.id}`"
              @click="remove(role)"
            />
          </template>
        </div>
        <ul v-if="role.permissions.length" class="perms">
          <li v-for="key in role.permissions" :key="key" class="chip" :title="permissionHint(key)">{{ permissionLabel(key) }}</li>
        </ul>
        <p v-else class="muted">{{ t('hub.roles.noPermissions') }}</p>
        <p v-if="!role.builtin && role.members > 0" class="muted" :data-testid="`hub-admin-role-delete-reason-${role.id}`">{{ t('hub.roles.deleteHeld', { count: role.members }) }}</p>
        <p v-if="errors.get(role.id)" class="error" role="alert" :data-testid="`hub-admin-role-error-${role.id}`">{{ errors.get(role.id) }}</p>
      </li>
    </ul>
    <form v-if="form" class="form" data-testid="hub-admin-role-form" @submit.prevent="save">
      <h4 class="form-title">{{ formTitle }}</h4>
      <label class="field">
        <span>{{ t('hub.roles.name') }}</span>
        <VInputText v-model="name" size="small" autocomplete="off" :placeholder="t('hub.roles.namePlaceholder')" :disabled="saving" data-testid="hub-admin-role-name" />
      </label>
      <fieldset v-for="group in groups" :key="group.id" class="group" :disabled="saving" :data-testid="`hub-admin-role-group-${group.id}`">
        <legend>{{ group.label }}</legend>
        <label v-for="key in group.keys" :key="key" class="perm">
          <UiInput type="checkbox" class="check" :checked="selected.has(key)" :data-testid="`hub-admin-role-perm-${key}`" @change="toggle(key, ($event.target as HTMLInputElement).checked)" />
          <span class="perm-text">
            <span class="perm-label">{{ permissionLabel(key) }}</span>
            <span class="muted">{{ permissionHint(key) }}</span>
          </span>
        </label>
      </fieldset>
      <p v-if="errors.get(FORM)" class="error" role="alert" data-testid="hub-admin-role-form-error">{{ errors.get(FORM) }}</p>
      <div class="actions">
        <VButton type="submit" :label="t('hub.roles.save')" size="small" :loading="saving" :disabled="!name.trim() || saving" data-testid="hub-admin-role-save" />
        <VButton :label="t('common.cancel')" size="small" severity="secondary" text :disabled="saving" data-testid="hub-admin-role-cancel" @click="form = null" />
      </div>
    </form>
  </section>
</template>

<style scoped>
.roles {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 0;
}

.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.muted {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  font-weight: 400;
  color: var(--vr-color-text-muted);
}

.list {
  display: flex;
  flex-direction: column;
  max-height: 28rem;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.role {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
}

.role + .role {
  border-top: 1px solid var(--vr-color-border);
}

.line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.name {
  flex: 1 1 10rem;
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.perms {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.chip {
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 32rem;
  padding: var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.form-title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  overflow-wrap: anywhere;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin: 0;
  padding: var(--vr-space-2) var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.group legend {
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  text-transform: uppercase;
}

.perm {
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-2);
}

.check {
  flex: none;
  margin: 2px 0 0;
  accent-color: var(--vr-color-accent);
}

.perm-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  font-size: var(--vr-font-size-sm);
}

.perm-label {
  font-weight: 500;
}

.error {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  gap: var(--vr-space-2);
}
</style>
