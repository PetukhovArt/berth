<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Trash2 } from '../../../shared/ui/icons';
import { VButton, VMessage } from '../../../shared/ui/kit';
import { describeError } from '../../../shared/api/connectors/api';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { megabytes, removeVoiceModels, voiceModels } from '../../../shared/lib/nativeVoice';

const { t } = useI18n();
const failure = shallowRef<string | null>(null);
const busy = shallowRef(false);

const models = computed(() => voiceModels.value?.models ?? []);
const downloading = computed(() => models.value.some((model) => model.received !== null));
const stored = computed(() => models.value.filter((model) => model.ready));
const lines = computed(() =>
  models.value.flatMap((model) => {
    const name = model.id === 'dictation' ? 'voice.models.dictation' : 'voice.models.wake';
    if (model.received !== null) return [`${t(name, { size: megabytes(model.bytes) })}, ${t('voice.models.downloading', { percent: Math.floor((model.received / Math.max(model.bytes, 1)) * 100) })}`];
    return model.ready ? [t(name, { size: megabytes(model.bytes) })] : [];
  }),
);
const errors = computed(() => models.value.flatMap((model) => (model.error ? [t('voice.models.failed', { error: model.error })] : [])));

async function remove() {
  busy.value = true;
  failure.value = null;
  try {
    await removeVoiceModels();
  } catch (e) {
    failure.value = describeError(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="row vr-field" data-testid="voice-models">
    <div class="meta">
      <span class="label">{{ t('voice.models.label') }}</span>
      <p class="description">{{ t('voice.models.description', { dir: voiceModels?.dir ?? '' }) }}</p>
      <p class="state" data-testid="voice-models-state">{{ lines.length ? lines.join(' · ') : t('voice.models.none') }}</p>
      <VMessage v-for="error in errors" :key="error" severity="warn" size="small" data-testid="voice-models-error">{{ error }}</VMessage>
      <VMessage v-if="failure" severity="error" size="small">{{ failure }}</VMessage>
    </div>
    <div class="control">
      <VButton size="small" severity="secondary" :label="t('voice.models.remove')" :disabled="busy || downloading || !stored.length" data-testid="voice-models-remove" @click="remove">
        <template #icon><Trash2 :size="14" /></template>
      </VButton>
    </div>
  </div>
</template>

<style scoped>
.row {
  padding: var(--vr-space-4) 0;
  border-bottom: 1px solid var(--vr-color-border);
}

.meta {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
}

.label {
  font-weight: 600;
}

.description,
.state {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  overflow-wrap: anywhere;
}

.control {
  display: flex;
  align-items: center;
}
</style>
