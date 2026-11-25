<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { VButton, VSelect } from '../../../shared/ui/kit';
import ConfirmPopover from '../../../shared/ui/ConfirmPopover.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import type { HubMember, HubRole } from '../../../entities/session';
import { useChanges } from '../model/changes';

const props = defineProps<{ members: HubMember[]; roles: HubRole[]; me: string }>();

const ORDER = { approved: 0, pending: 1, revoked: 2 } as const;

const { busy, errors, run } = useChanges();
const confirm = useTemplateRef<InstanceType<typeof ConfirmPopover>>('confirm');
const revoking = shallowRef<HubMember | null>(null);
const revision = shallowRef(0);

const rows = computed(() => [...props.members].sort((a, b) => ORDER[a.status] - ORDER[b.status]));
const waiting = computed(() => props.members.filter((member) => member.status === 'pending').length);

async function setRole(member: HubMember, role: string) {
  if (role === member.role) return;
  if (!(await run(member.id, { SetRole: { member: member.id, role } }))) revision.value++;
}

function askRevoke(member: HubMember, event: MouseEvent) {
  const target = event.currentTarget as HTMLElement;
  revoking.value = member;
  setTimeout(() => confirm.value?.show(target), 0);
}

async function confirmRevoke() {
  const member = revoking.value;
  if (!member) return;
  await run(member.id, { Revoke: { member: member.id } });
  confirm.value?.hide();
}
</script>

<template>
  <section class="members" data-testid="hub-admin-members">
    <h3 class="head">
      {{ t('hub.members.title') }}
      <span v-if="waiting" class="waiting" data-testid="hub-admin-members-waiting">{{ t('hub.members.waiting', { count: waiting }) }}</span>
    </h3>
    <p v-if="!rows.length" class="muted" data-testid="hub-admin-members-empty">{{ t('hub.members.empty') }}</p>
    <ul v-else class="list" data-testid="hub-admin-members-list">
      <li v-for="member in rows" :key="member.id" class="row" :data-status="member.status" :data-testid="`hub-admin-member-${member.id}`">
        <span class="name" :title="member.name">{{ member.id === me ? t('hub.members.you', { name: member.name }) : member.name }}</span>
        <span class="status" :data-testid="`hub-admin-member-status-${member.id}`">{{ t(`hub.members.status.${member.status}`) }}</span>
        <template v-if="member.status !== 'revoked'">
          <VSelect
            :key="`${member.id}:${revision}`"
            :model-value="member.role"
            :options="roles"
            option-label="name"
            option-value="id"
            size="small"
            class="role"
            :placeholder="t('hub.members.approveAs')"
            :aria-label="t('hub.members.role', { name: member.name })"
            :disabled="busy.has(member.id) || member.id === me"
            :title="member.id === me ? t('hub.members.self') : undefined"
            :data-testid="`hub-admin-member-role-${member.id}`"
            @update:model-value="setRole(member, $event)"
          />
          <VButton
            :label="t('hub.members.revoke')"
            size="small"
            severity="danger"
            text
            :disabled="busy.has(member.id) || member.id === me"
            :title="member.id === me ? t('hub.members.self') : undefined"
            :data-testid="`hub-admin-member-revoke-${member.id}`"
            @click="askRevoke(member, $event)"
          />
        </template>
        <p v-if="errors.get(member.id)" class="error" role="alert" :data-testid="`hub-admin-member-error-${member.id}`">{{ errors.get(member.id) }}</p>
      </li>
    </ul>
    <ConfirmPopover
      ref="confirm"
      :question="t('hub.members.revokeQuestion', { name: revoking?.name ?? '' })"
      :confirm-label="t('hub.members.revokeConfirm')"
      :busy="revoking ? busy.has(revoking.id) : false"
      testid="hub-admin-member-revoke-confirm"
      @confirm="confirmRevoke"
      @cancel="revoking = null"
    />
  </section>
</template>

<style scoped>
.members {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 0;
}

.head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.waiting {
  font-size: var(--vr-font-size-xs);
  font-weight: 400;
  color: var(--vr-status-feedback);
}

.muted {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.list {
  display: flex;
  flex-direction: column;
  max-height: 24rem;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
}

.row + .row {
  border-top: 1px solid var(--vr-color-border);
}

.row[data-status='revoked'] {
  color: var(--vr-color-text-muted);
  opacity: 0.6;
}

.name {
  flex: 1 1 10rem;
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status {
  flex: none;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.row[data-status='pending'] .status {
  color: var(--vr-status-feedback);
}

.role {
  flex: none;
  width: 12rem;
}

.error {
  flex: 1 0 100%;
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
  overflow-wrap: anywhere;
}
</style>
