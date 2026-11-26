<script setup lang="ts">
import { computed } from 'vue';
import { allowRequest, alwaysAllowRequest, chooseOption, denyRequest, feedbackText, isYesNoPermission, offersAlways, StatusIcon, useBerth, useFeedbackRequest, type SessionRef } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { VButton, vTooltip } from '../../../shared/ui/kit';

const props = defineProps<{ session: SessionRef }>();
const request = useFeedbackRequest(props.session);
const text = computed(() => (request.value ? feedbackText(request.value) : ''));
const berth = useBerth();
const place = computed(() => {
  const entities = berth?.entities.value;
  const agent = 'Agent' in props.session ? entities?.agents.get(props.session.Agent) : undefined;
  const worktree = agent ? entities?.worktrees.get(agent.worktree_id) : undefined;
  const project = worktree ? entities?.projects.get(worktree.project_id) : undefined;
  return [project?.name, worktree?.branch].filter(Boolean).join(' › ');
});
</script>

<template>
  <div v-if="request" class="strip" data-testid="feedback-strip" :data-kind="request.kind">
    <span class="grip vr-phone-only" aria-hidden="true" />
    <StatusIcon class="icon" status="needs_feedback" />
    <span class="text" data-testid="feedback-text">{{ text }}</span>
    <code v-if="request.detail" class="detail" data-testid="feedback-detail">{{ request.detail }}</code>
    <span v-if="place" class="place vr-phone-only" data-testid="feedback-place">{{ t('feedback.place', { place }) }}</span>
    <span class="hint vr-wide-only">{{ t('feedback.orTerminal') }}</span>
    <div class="actions">
      <template v-if="isYesNoPermission(request)">
        <VButton class="action" :label="t('feedback.allow')" size="small" data-testid="feedback-allow" @click="allowRequest(session)" />
        <VButton v-if="offersAlways(request)" v-tooltip="t('feedback.alwaysTitle')" class="action" :label="t('feedback.always')" severity="secondary" size="small" data-testid="feedback-always" @click="alwaysAllowRequest(session)" />
        <VButton class="action" :label="t('feedback.deny')" severity="secondary" size="small" data-testid="feedback-deny" @click="denyRequest(session)" />
      </template>
      <VButton v-for="(option, index) in request.options" v-else :key="index" class="action" :label="option" :severity="index === 0 ? undefined : 'secondary'" size="small" data-testid="feedback-option" @click="chooseOption(session, index)" />
    </div>
  </div>
</template>

<style scoped>
.strip {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  min-height: 46px;
  padding: 7px 12px 7px 16px;
  background: var(--vr-tint-feedback);
  border-top: 1px solid var(--vr-color-border);
}

.icon {
  flex: none;
  color: var(--vr-status-feedback);
}

.text {
  flex: none;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
}

.detail {
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  max-height: min(30dvh, 10em);
  padding: 1px 6px;
  overflow-x: hidden;
  overflow-y: auto;
  overflow-wrap: anywhere;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: pre-wrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
}

.hint {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.actions {
  display: flex;
  flex: none;
  gap: 10px;
}

.action {
  flex: none;
}

:root[data-vr-viewport='phone'] .strip {
  position: relative;
  gap: var(--vr-space-2) var(--vr-space-2);
  padding: var(--vr-space-4) var(--vr-space-3) var(--vr-space-3);
  background: var(--vr-color-surface-raised);
  border-top: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-2xl) var(--vr-radius-2xl) 0 0;
  box-shadow: var(--vr-shadow-popover);
}

.grip {
  position: absolute;
  top: var(--vr-space-1);
  left: 50%;
  width: 36px;
  height: 4px;
  background: var(--vr-color-border-strong);
  border-radius: var(--vr-radius-full);
  transform: translateX(-50%);
}

:root[data-vr-viewport='phone'] .text {
  flex: 1;
  min-width: 0;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
}

:root[data-vr-viewport='phone'] .detail {
  flex: 1 1 100%;
  max-height: min(24dvh, 8em);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  background: var(--vr-color-surface-sunken);
  border-radius: var(--vr-radius-md);
}

.place {
  flex: 1 1 100%;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

:root[data-vr-viewport='phone'] .actions {
  flex: 1 1 100%;
  gap: var(--vr-space-2);
}

:root[data-vr-viewport='phone'] .action {
  flex: 1;
  min-height: var(--vr-size-touch);
}
</style>
