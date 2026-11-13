<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VButton, VInputText, VMessage, VToggleSwitch } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { reloadProviders, removeAcpProvider, saveAcpProvider, setProviderEnabled, useProviders, type Provider } from '../../../shared/api/providers';

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

const NEW = '';
const editing = shallowRef<string | null>(null);
const draftId = shallowRef('');
const draftName = shallowRef('');
const draftCommand = shallowRef('');
const draftArgs = shallowRef('');

async function run(task: () => Promise<void>): Promise<boolean> {
  try {
    await task();
    error.value = null;
    return true;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    return false;
  }
}

function toggle(provider: Provider, enabled: boolean) {
  void run(() => setProviderEnabled(provider, enabled));
}

function startEdit(provider: Provider | null) {
  editing.value = provider?.id ?? NEW;
  draftId.value = provider?.id ?? '';
  draftName.value = provider && !provider.built_in ? provider.name : '';
  draftCommand.value = provider?.program ?? '';
  draftArgs.value = (provider?.args ?? []).join(' ');
}

async function saveEdit() {
  const saved = await run(() =>
    saveAcpProvider({ id: draftId.value.trim(), name: draftName.value.trim(), command: draftCommand.value.trim(), args: draftArgs.value.split(/\s+/).filter(Boolean) }),
  );
  if (saved) editing.value = null;
}

async function remove(provider: Provider) {
  const removed = await run(() => removeAcpProvider(provider.id));
  if (removed) editing.value = null;
}
</script>

<template>
  <div class="providers" data-testid="providers-panel">
    <VMessage v-if="error" severity="error" size="small" data-testid="providers-error">{{ error }}</VMessage>
    <section v-for="section in sections" :key="section.id" class="section" :data-testid="`providers-${section.id}`">
      <h2 class="title">{{ section.title }}</h2>
      <p class="hint">{{ section.hint }}</p>
      <div v-for="provider in section.rows" :key="provider.id" class="row" :data-testid="`provider-${provider.acp ? 'acp' : 'native'}-${provider.id}`" :data-enabled="provider.enabled">
        <div class="meta">
          <label class="label" :for="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`">{{ provider.name }}</label>
          <code v-if="provider.command" class="command">{{ provider.command }}</code>
          <span v-if="provider.found === false" class="missing" data-testid="provider-not-found">{{ t('settings.providers.notFound') }}</span>
          <p v-if="provider.acp && provider.id === PI" class="warning" data-testid="provider-pi-warning">{{ t('settings.providers.piWarning') }}</p>
          <div v-if="provider.acp && editing !== provider.id" class="row-actions">
            <VButton :label="t('settings.providers.edit')" size="small" text :data-testid="`provider-edit-${provider.id}`" @click="startEdit(provider)" />
          </div>
          <form v-if="provider.acp && editing === provider.id" class="editor" :data-testid="`provider-editor-${provider.id}`" @submit.prevent="saveEdit">
            <label v-if="!provider.built_in" class="field"><span>{{ t('settings.providers.name') }}</span><VInputText v-model="draftName" size="small" data-testid="provider-draft-name" /></label>
            <label class="field"><span>{{ t('settings.providers.command') }}</span><VInputText v-model="draftCommand" size="small" data-testid="provider-draft-command" /></label>
            <label class="field"><span>{{ t('settings.providers.args') }}</span><VInputText v-model="draftArgs" size="small" data-testid="provider-draft-args" /></label>
            <p class="hint">{{ t('settings.providers.argsHint') }}</p>
            <div class="row-actions">
              <VButton type="submit" :label="t('settings.providers.save')" size="small" :disabled="!draftCommand.trim()" data-testid="provider-draft-save" />
              <VButton :label="provider.built_in ? t('settings.providers.reset') : t('settings.providers.remove')" size="small" severity="secondary" outlined :data-testid="`provider-remove-${provider.id}`" @click="remove(provider)" />
              <VButton :label="t('settings.providers.cancel')" size="small" text data-testid="provider-draft-cancel" @click="editing = null" />
            </div>
          </form>
        </div>
        <VToggleSwitch
          :input-id="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`"
          :model-value="provider.enabled"
          :data-testid="`provider-toggle-${provider.acp ? 'acp' : 'native'}-${provider.id}`"
          @update:model-value="toggle(provider, $event)"
        />
      </div>
      <template v-if="section.id === 'acp'">
        <form v-if="editing === NEW" class="editor" data-testid="provider-editor-new" @submit.prevent="saveEdit">
          <label class="field"><span>{{ t('settings.providers.id') }}</span><VInputText v-model="draftId" size="small" data-testid="provider-draft-id" /></label>
          <label class="field"><span>{{ t('settings.providers.name') }}</span><VInputText v-model="draftName" size="small" data-testid="provider-draft-name" /></label>
          <label class="field"><span>{{ t('settings.providers.command') }}</span><VInputText v-model="draftCommand" size="small" data-testid="provider-draft-command" /></label>
          <label class="field"><span>{{ t('settings.providers.args') }}</span><VInputText v-model="draftArgs" size="small" data-testid="provider-draft-args" /></label>
          <p class="hint">{{ t('settings.providers.argsHint') }} {{ t('settings.providers.secretsHint') }}</p>
          <div class="row-actions">
            <VButton type="submit" :label="t('settings.providers.save')" size="small" :disabled="!draftId.trim() || !draftCommand.trim()" data-testid="provider-draft-save" />
            <VButton :label="t('settings.providers.cancel')" size="small" text data-testid="provider-draft-cancel" @click="editing = null" />
          </div>
        </form>
        <div v-else class="row-actions">
          <VButton :label="t('settings.providers.add')" size="small" outlined data-testid="provider-add" @click="startEdit(null)" />
        </div>
      </template>
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
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
}

.row > .p-toggleswitch {
  flex: none;
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

.row-actions {
  display: flex;
  gap: var(--vr-space-2);
  padding-top: var(--vr-space-1);
}

.editor {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
}

.warning {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
