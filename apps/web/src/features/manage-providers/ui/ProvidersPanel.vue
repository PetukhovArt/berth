<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VMessage, VToggleSwitch } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { reloadProviders, setProviderEnabled, useProviders, type Provider } from '../../../shared/api/providers';

const PI = 'pi';

const { t } = useI18n();
const providers = useProviders();
const error = shallowRef<string | null>(null);
const native = computed(() => providers.value.filter((provider) => !provider.acp));
const acp = computed(() => providers.value.filter((provider) => provider.acp));
const sections = computed(() => [
  { id: 'native', title: t('settings.providers.native'), hint: t('settings.providers.nativeHint'), rows: native.value },
  { id: 'acp', title: t('settings.providers.acp'), hint: t('settings.providers.acpHint'), rows: acp.value },
]);

void reloadProviders();

async function toggle(provider: Provider, enabled: boolean) {
  try {
    await setProviderEnabled(provider, enabled);
    error.value = null;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <div class="providers" data-testid="providers-panel">
    <VMessage v-if="error" severity="error" size="small" data-testid="providers-error">{{ error }}</VMessage>
    <section v-for="section in sections" :key="section.id" class="section" :data-testid="`providers-${section.id}`">
      <h2 class="title">{{ section.title }}</h2>
      <p class="hint">{{ section.hint }}</p>
      <div v-for="provider in section.rows" :key="provider.id" class="row vr-field" :data-testid="`provider-${provider.acp ? 'acp' : 'native'}-${provider.id}`" :data-enabled="provider.enabled">
        <div class="meta">
          <label class="label" :for="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`">{{ provider.name }}</label>
          <code v-if="provider.command" class="command">{{ provider.command }}</code>
          <span v-if="provider.found === false" class="missing" data-testid="provider-not-found">{{ t('settings.providers.notFound') }}</span>
          <p v-if="provider.acp && provider.id === PI" class="warning" data-testid="provider-pi-warning">{{ t('settings.providers.piWarning') }}</p>
        </div>
        <VToggleSwitch
          :input-id="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`"
          :model-value="provider.enabled"
          :data-testid="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`"
          @update:model-value="toggle(provider, $event)"
        />
      </div>
    </section>
  </div>
</template>

<style scoped>
.providers {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  margin: var(--vr-space-1) 0 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) 0;
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

.command {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.missing {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.warning {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
