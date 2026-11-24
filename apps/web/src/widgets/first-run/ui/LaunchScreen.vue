<script setup lang="ts">
import { t } from '../../../shared/lib/i18n/i18n';
import { copyText } from '../../../shared/lib/atlassian';
import BrandMark from '../../../shared/ui/BrandMark.vue';
import { VButton } from '../../../shared/ui/kit';
import type { Launch } from '../model/launch';

const props = defineProps<{ launch: Exclude<Launch, { kind: 'ready' }> }>();

const copy = () => (props.launch.kind === 'starting' || !props.launch.detail ? Promise.resolve() : copyText(props.launch.detail));
</script>

<template>
  <section class="launch" data-testid="launch-screen" :data-state="launch.kind">
    <div v-if="launch.kind === 'starting'" class="starting" role="status">
      <BrandMark size="44px" class="mark" />
      <p class="label" data-testid="launch-starting">{{ t('workbench.launch.starting') }}</p>
      <span class="track" aria-hidden="true"><span class="fill" /></span>
    </div>
    <div v-else class="card" role="alert">
      <BrandMark size="36px" class="mark" />
      <h1 class="title" data-testid="launch-title">{{ launch.kind === 'offline' ? t('workbench.launch.offlineTitle') : t('workbench.launch.incompatibleTitle') }}</h1>
      <p class="body">{{ launch.kind === 'offline' ? t('workbench.launch.offlineBody') : t('workbench.launch.incompatibleBody') }}</p>
      <pre v-if="launch.detail" class="detail" data-testid="launch-detail">{{ launch.detail }}</pre>
      <div v-if="launch.detail" class="actions">
        <VButton :label="t('workbench.launch.copyDetails')" severity="secondary" data-testid="launch-copy" @click="copy" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.launch {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  min-height: 0;
  min-width: 0;
}

.starting {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 18px;
}

.label {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.track {
  width: 160px;
  height: 2px;
  overflow: hidden;
  background: var(--vr-color-border);
  border-radius: 1px;
}

.fill {
  display: block;
  width: 45%;
  height: 100%;
  background: var(--vr-color-text-muted);
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: min(480px, 100% - 2 * var(--vr-space-4));
  padding: 28px;
  background: var(--vr-color-surface);
  border-radius: 14px;
  box-shadow: var(--vr-shadow-dialog);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  text-wrap: pretty;
  color: var(--vr-color-text);
}

.body {
  margin: 0;
  font-size: var(--vr-font-size-label);
  line-height: 1.5;
  color: var(--vr-color-text-muted);
}

.detail {
  margin: 0;
  padding: 10px var(--vr-space-3);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  line-height: 1.6;
  color: var(--vr-color-text-muted);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  background: var(--vr-color-surface-sunken);
  border-radius: var(--vr-radius-md);
}

.actions {
  display: flex;
  gap: var(--vr-space-2);
}
</style>
