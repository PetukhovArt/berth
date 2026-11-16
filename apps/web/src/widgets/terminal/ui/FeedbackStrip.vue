<script setup lang="ts">
import { computed } from 'vue';
import { allowRequest, chooseOption, denyRequest, feedbackText, StatusIcon, useFeedbackRequest, type SessionRef } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { UiButton } from '../../../shared/ui/UiButton';

const props = defineProps<{ session: SessionRef }>();
const request = useFeedbackRequest(props.session);
const text = computed(() => (request.value ? feedbackText(request.value) : ''));
</script>

<template>
  <div v-if="request" class="strip" data-testid="feedback-strip" :data-kind="request.kind">
    <StatusIcon class="icon" status="needs_feedback" />
    <span class="text" data-testid="feedback-text">{{ text }}</span>
    <code v-if="request.detail" class="detail" data-testid="feedback-detail">{{ request.detail }}</code>
    <span class="hint">{{ t('feedback.orTerminal') }}</span>
    <template v-if="request.kind === 'Permission'">
      <UiButton type="button" class="action primary" data-testid="feedback-allow" @click="allowRequest(session)">{{ t('feedback.allow') }}</UiButton>
      <UiButton type="button" class="action" data-testid="feedback-deny" @click="denyRequest(session)">{{ t('feedback.deny') }}</UiButton>
    </template>
    <UiButton v-for="(option, index) in request.options" v-else :key="index" type="button" class="action" :class="{ primary: index === 0 }" data-testid="feedback-option" @click="chooseOption(session, index)">{{ option }}</UiButton>
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

.action {
  flex: none;
  height: 28px;
  padding: 0 12px;
  font-size: var(--vr-font-size-sm);
  font-weight: 500;
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
}

:global(:root[data-vr-viewport='phone']) .detail {
  flex: 1 1 100%;
  padding: 6px 8px;
}

.action.primary {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}
</style>
