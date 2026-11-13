<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { VContextMenu } from '../../../shared/ui/kit';
import { copyText } from '../../../shared/lib/atlassian';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { archiveExternal, externalDot, externalOrigin, externalStatusLabel, resumeCommand, type ExternalSession, StatusIcon, useAction } from '../../../entities/session';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ session: ExternalSession; active: boolean }>();
const emit = defineEmits<{ open: [split: boolean] }>();

const { t, formatRelative, formatDate } = useI18n();
const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const action = useAction();

const origin = computed(() => externalOrigin(props.session));
const tooltip = computed(() =>
  [props.session.title, `${externalStatusLabel(props.session)} · ${formatDate(props.session.updated_at)}`, props.session.branch ? `${props.session.cwd} · ${props.session.branch}` : props.session.cwd, origin.value].join('\n'),
);
const menuItems = computed(() => [
  { key: 'open', label: t('workbench.external.open'), command: () => emit('open', false) },
  { key: 'copy', label: t('workbench.external.copyResume'), command: () => void copyText(resumeCommand(props.session)) },
  { key: 'archive', label: t('agents.session.archive'), command: () => void action.run(() => archiveExternal(props.session)) },
]);
</script>

<template>
  <div class="external">
    <UiButton
      class="row status-host"
      data-testid="external-session-row"
      :data-session-id="session.id"
      :data-harness="session.harness"
      :data-status="session.status"
      :data-running="session.running"
      :data-active="active"
      :title="tooltip"
      @click="emit('open', primaryPressed($event))"
      @contextmenu.prevent="menu?.show($event)"
    >
      <StatusIcon :status="externalDot(session)" :unseen="false" />
      <span class="name">{{ session.title }}</span>
      <span class="label">{{ session.running ? externalStatusLabel(session) : formatRelative(session.updated_at) }}</span>
    </UiButton>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`external-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <p v-if="action.error.value" class="row-error" role="alert" data-testid="external-error">{{ action.error.value }}</p>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  height: 28px;
  padding: 0 4px 0 var(--vr-space-5);
  font: inherit;
  font-size: 13px;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.row[data-running='false'] .name {
  color: var(--vr-color-text-muted);
}

.row[data-active='true'],
.row[data-active='true'] .name {
  color: var(--vr-color-text);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.label {
  flex: none;
  font-size: 11.5px;
  color: var(--vr-color-text-muted);
}

.row[data-running='true'][data-status='waiting'] .label {
  color: var(--vr-status-feedback);
}

.row-error {
  margin: 0;
  padding: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
}
</style>
