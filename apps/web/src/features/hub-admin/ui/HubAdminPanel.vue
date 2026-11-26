<script setup lang="ts">
import { onMounted } from 'vue';
import { VButton, VMessage, VTag, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { permissionHint, permissionLabel, refreshHub, useHubAdmin } from '../../../entities/session';
import HubAudit from './HubAudit.vue';
import HubMembers from './HubMembers.vue';
import HubRoles from './HubRoles.vue';

const { me, admin, state, failure, refreshing } = useHubAdmin();

onMounted(() => void refreshHub());
</script>

<template>
  <div class="hub" data-testid="hub-admin-panel" :data-state="state">
    <h2 class="title">{{ t('hub.title') }}</h2>
    <p v-if="state === 'loading'" class="muted" role="status" data-testid="hub-admin-state-loading">{{ t('hub.state.loading') }}</p>
    <div v-else-if="state === 'off'" class="block" data-testid="hub-admin-state-off">
      <p class="muted">{{ t('hub.state.off') }}</p>
      <p class="keys"><code>hub_url</code><code>hub_token</code></p>
    </div>
    <p v-else-if="state === 'legacy'" class="muted" data-testid="hub-admin-state-legacy">{{ t('hub.state.legacy') }}</p>
    <div v-else-if="state === 'unreachable'" class="block" data-testid="hub-admin-state-unreachable">
      <VMessage severity="error" size="small" data-testid="hub-admin-error">{{ t('hub.state.unreachable', { error: failure ?? '' }) }}</VMessage>
      <div>
        <VButton :label="t('common.retry')" size="small" severity="secondary" :loading="refreshing" :disabled="refreshing" data-testid="hub-admin-retry" @click="refreshHub" />
      </div>
    </div>
    <template v-else-if="me">
      <div class="block" data-testid="hub-admin-me">
        <p v-tooltip="me.name" class="who">{{ t('hub.me.who', { name: me.name }) }}</p>
        <p class="muted" data-testid="hub-admin-me-role">{{ t('hub.me.role', { role: me.role.name }) }}</p>
      </div>
      <div v-if="state === 'member'" class="block" data-testid="hub-admin-state-member">
        <p class="muted">{{ t(me.permissions.length ? 'hub.me.grants' : 'hub.me.grantsNone') }}</p>
        <ul v-if="me.permissions.length" class="grants" data-testid="hub-admin-me-permissions">
          <li v-for="key in me.permissions" :key="key"><VTag v-tooltip="permissionHint(key)" :value="permissionLabel(key)" severity="secondary" rounded :data-testid="`hub-admin-me-permission-${key}`" /></li>
        </ul>
        <p class="muted">{{ t('hub.me.notAdmin') }}</p>
      </div>
      <template v-else-if="admin">
        <div class="sections" data-testid="hub-admin-state-admin">
          <div>
            <VButton :label="t('common.refresh')" size="small" severity="secondary" :loading="refreshing" :disabled="refreshing" data-testid="hub-admin-refresh" @click="refreshHub" />
          </div>
          <HubMembers :members="admin.members" :roles="admin.roles" :me="me.id" />
          <HubRoles :roles="admin.roles" :catalog="admin.catalog" />
          <HubAudit :admin="admin" />
        </div>
      </template>
    </template>
  </div>
</template>

<style scoped>
.hub {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  min-width: 0;
  padding: var(--vr-space-2) 0;
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.muted {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
}

.who {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.keys {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  margin: 0;
}

.grants {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.sections {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-5);
  min-width: 0;
  max-width: 47.5rem;
}
</style>
