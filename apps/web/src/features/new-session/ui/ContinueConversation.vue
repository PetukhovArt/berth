<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VInputText } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon } from '../../../entities/session';
import type { ConversationChoice, ProjectConversations } from '../model/conversations';

const props = defineProps<{ conversations: ProjectConversations }>();
const picked = defineModel<string | null>({ required: true });
const { t, formatRelative } = useI18n();
const filter = shallowRef('');
const { choices, loading, error } = props.conversations;

const shown = computed(() => {
  const words = filter.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return choices.value;
  return choices.value.filter((choice) => {
    const text = `${choice.title} ${choice.checkout} ${choice.harness}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
});

function pick(choice: ConversationChoice) {
  if (choice.busy) return;
  picked.value = picked.value === choice.key ? null : choice.key;
}
</script>

<template>
  <div class="continue" data-testid="new-session-continue-list">
    <VInputText v-model="filter" size="small" class="filter" :placeholder="t('agents.newSession.continueFilter')" :aria-label="t('agents.newSession.continueFilter')" data-testid="new-session-continue-filter" />
    <p v-if="error" class="state error" role="alert">{{ error }}</p>
    <p v-else-if="loading && !choices.length" class="state">{{ t('agents.newSession.continueLoading') }}</p>
    <p v-else-if="!choices.length" class="state">{{ t('agents.newSession.continueEmpty') }}</p>
    <p v-else-if="!shown.length" class="state">{{ t('agents.newSession.continueNoMatch') }}</p>
    <ul v-else class="list" role="listbox" :aria-label="t('agents.newSession.continueMode')">
      <li v-for="choice in shown" :key="choice.key" role="presentation">
        <UiButton
          type="button"
          class="item"
          role="option"
          :aria-selected="picked === choice.key"
          :aria-disabled="!!choice.busy"
          :data-selected="picked === choice.key"
          :data-busy="!!choice.busy"
          :title="choice.busy ?? choice.title"
          data-testid="new-session-continue-item"
          :data-session-id="choice.id"
          @click="pick(choice)"
        >
          <HarnessIcon :harness="choice.harness" :size="13" />
          <span class="title">{{ choice.title }}</span>
          <span class="meta">
            <span v-if="choice.busy" class="busy">{{ choice.busy }}</span>
            <template v-else>
              <span v-if="choice.checkout" class="checkout mono">{{ choice.checkout }}</span>
              <span v-if="choice.at" class="age">{{ formatRelative(choice.at) }}</span>
            </template>
          </span>
        </UiButton>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.continue {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 12px;
  background: var(--vr-color-surface-sunken);
}

.filter {
  width: 100%;
}

.list {
  display: flex;
  flex-direction: column;
  max-height: 15rem;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-label);
  text-align: left;
  color: var(--vr-color-text);
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.item:hover {
  background: var(--vr-color-nav-hover);
}

.item[data-selected='true'] {
  background: var(--vr-color-nav-active);
}

.item[data-busy='true'] {
  cursor: not-allowed;
  color: var(--vr-color-text-muted);
}

.item[data-busy='true']:hover {
  background: none;
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  display: inline-flex;
  flex: none;
  gap: var(--vr-space-2);
  max-width: 50%;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
}

.checkout {
  overflow: hidden;
  text-overflow: ellipsis;
}

.mono {
  font-family: var(--vr-font-mono);
}

.busy {
  color: var(--vr-status-feedback);
}

.state {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.state.error {
  color: var(--vr-color-danger);
}
</style>
