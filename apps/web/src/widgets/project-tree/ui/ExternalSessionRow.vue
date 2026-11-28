<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { VContextMenu, VMessage, VMiddleTruncate, VRowButton, vTooltip } from '../../../shared/ui/kit';
import { copyText } from '../../../shared/lib/atlassian';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { archiveExternal, externalOrigin, resumeCommand, type ExternalSession, StatusDot, useAction } from '../../../entities/session';
const props = defineProps<{ session: ExternalSession; active: boolean; when?: string }>();
const emit = defineEmits<{ open: [split: boolean] }>();

const { t, formatRelative, formatDate } = useI18n();
const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const action = useAction();

const origin = computed(() => externalOrigin(props.session));
const tooltip = computed(() =>
  [props.session.title, `${t(`agents.status.${props.session.status}`)} · ${formatDate(props.session.updated_at)}`, props.session.branch ? `${props.session.cwd} · ${props.session.branch}` : props.session.cwd, origin.value].join('\n'),
);
const menuItems = computed(() => [
  { key: 'open', label: t('workbench.external.open'), command: () => emit('open', false) },
  { key: 'copy', label: t('workbench.external.copyResume'), command: () => void copyText(resumeCommand(props.session)) },
  { key: 'archive', label: t('agents.session.archive'), command: () => void action.run(() => archiveExternal(props.session)) },
]);
</script>

<template>
  <div class="external">
    <VRowButton
      v-tooltip="tooltip"
      class="row status-host"
      data-testid="external-session-row"
      :data-session-id="session.id"
      :data-harness="session.harness"
      :data-status="session.status"
      :data-running="session.running"
      :data-current="active || undefined"
      @click="emit('open', primaryPressed($event))"
      @contextmenu.prevent="menu?.show($event)"
    >
      <template #leading><StatusDot :status="session.status" :unseen="false" /></template>
      <VMiddleTruncate class="name" :text="session.title" />
      <template #trailing><span class="label">{{ session.running ? t(`agents.status.${session.status}`) : (when ?? formatRelative(session.updated_at)) }}</span></template>
    </VRowButton>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`external-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <VMessage v-if="action.error.value" class="row-error" severity="error" size="small" data-testid="external-error">{{ action.error.value }}</VMessage>
  </div>
</template>

<style scoped>
.row[data-running='false'] .name {
  color: var(--vr-color-text-muted);
}

.row[data-current],
.row[data-current] .name {
  color: var(--vr-color-text);
}

.label {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.row[data-running='true'][data-status='waiting'] .label {
  color: var(--vr-status-feedback);
}

.row-error {
  margin: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
}
</style>
