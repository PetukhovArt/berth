<script setup lang="ts">
import { onMounted, shallowRef } from 'vue';
import { Bell, BellOff } from '../../../shared/ui/icons';
import { VButton, VMessage } from '../../../shared/ui/kit';
import { describeError } from '../../../shared/api/connectors/api';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { disablePush, enablePush, pushState, refreshPushState } from '../model/push';

const { t } = useI18n();
const failure = shallowRef<string | null>(null);
const busy = shallowRef(false);

async function run(action: () => Promise<void>) {
  busy.value = true;
  failure.value = null;
  try {
    await action();
  } catch (e) {
    failure.value = describeError(e);
  } finally {
    busy.value = false;
  }
}

onMounted(() => void run(refreshPushState));
</script>

<template>
  <div class="push" data-testid="web-push">
    <h3>{{ t('push.title') }}</h3>
    <p class="muted">{{ t(`push.${pushState}`) }}</p>
    <VMessage v-if="failure" severity="error" size="small">{{ failure }}</VMessage>
    <VButton v-if="pushState === 'off'" size="small" :label="t('push.enable')" :disabled="busy" data-testid="web-push-enable" @click="run(enablePush)">
      <template #icon><Bell :size="14" /></template>
    </VButton>
    <VButton v-else-if="pushState === 'on'" size="small" severity="secondary" :label="t('push.disable')" :disabled="busy" data-testid="web-push-disable" @click="run(disablePush)">
      <template #icon><BellOff :size="14" /></template>
    </VButton>
  </div>
</template>

<style scoped>
.push {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--vr-space-2);
}

.muted {
  margin: 0;
  color: var(--vr-color-text-muted);
}
</style>
