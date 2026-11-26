<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { shortcutLabel } from '../../../shared/lib/commands/keymap';
import { useSettings } from '../../../shared/config/settings/store';
import { defaultChatAgent, enabledNativeAgent, useProviders } from '../../../shared/api/providers';
import { ArrowUp, ChevronDown } from '../../../shared/ui/icons';
import { VButton, VTextarea, vTooltip } from '../../../shared/ui/kit';
import { AttachmentChip, uploadName, uploadSrc, useAttachments } from '../../../entities/session';
import { defaultMainTarget, startCreate, startCreateInMain } from '../../../features/new-session';

const target = computed(() => defaultMainTarget());
const providers = useProviders();
const { values } = useSettings();
const prompt = shallowRef('');
const notice = shallowRef('');
const attachments = useAttachments(
  () => true,
  (text) => (notice.value = text),
);
const takesFiles = computed(() => !!defaultChatAgent(values.value['sessions.default_kind'], providers.value));

const agent = computed(() => {
  const kind = values.value['sessions.default_kind'];
  if (kind === 'terminal') return t('agents.newSession.terminal');
  const chat = defaultChatAgent(kind, providers.value);
  const id = enabledNativeAgent(kind, providers.value);
  const name = chat?.name ?? providers.value.find((provider) => provider.id === id)?.name ?? kind;
  const model = values.value['sessions.default_model'];
  return model ? `${name} · ${model}` : name;
});

const chips = computed(() => [target.value?.project.name ?? '', target.value?.worktree.branch ?? '', agent.value].filter(Boolean));
const keys = computed(() => [
  shortcutLabel('sidebar.search') ? t('workbench.pickKeys.search', { key: shortcutLabel('sidebar.search') }) : '',
  shortcutLabel('agents.newSession') ? t('workbench.pickKeys.newSession', { key: shortcutLabel('agents.newSession') }) : '',
].filter(Boolean));

function launch(autoStart: boolean) {
  return { prompt: prompt.value.trim() || undefined, uploads: attachments.uploads.value, autoStart };
}

function start() {
  startCreateInMain(launch(true));
}

function openOptions() {
  startCreate(target.value?.worktree.id ?? null, false, launch(false));
}

function onDrop(event: DragEvent) {
  const files = event.dataTransfer?.files;
  if (!files?.length) return;
  notice.value = '';
  if (takesFiles.value) void attachments.add(Array.from(files));
  else notice.value = t('workbench.startCard.noFiles');
}

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  start();
}
</script>

<template>
  <div class="start" data-testid="dock-start">
    <div v-if="target" class="card-wrap" data-testid="dock-start-card">
      <h2 class="title">{{ t('workbench.startCard.title') }}</h2>
      <div class="card" data-testid="dock-start-box" @dragover.prevent @drop.prevent="onDrop">
        <div v-if="attachments.pending.value.length" class="files" data-testid="dock-start-attachments">
          <AttachmentChip v-for="entry in attachments.pending.value" :key="entry.id" :label="uploadName(entry.upload)" :src="uploadSrc(entry.upload)" removable @remove="attachments.remove(entry.id)" />
        </div>
        <VTextarea v-model="prompt" rows="3" auto-resize class="prompt" :aria-label="t('workbench.startCard.placeholder')" :placeholder="t('workbench.startCard.placeholder')" data-testid="dock-start-prompt" @keydown="onKey" />
        <div class="bar">
          <VButton v-for="chip in chips" :key="chip" v-tooltip="t('workbench.startCard.options')" :label="chip" icon-pos="right" severity="secondary" size="small" data-testid="dock-start-chip" @click="openOptions">
            <template #icon><ChevronDown :size="10" :stroke-width="1.8" /></template>
          </VButton>
          <span class="spacer" />
          <span class="enter">{{ t('workbench.startCard.enter') }}</span>
          <VButton v-tooltip="t('workbench.startCard.start')" rounded :aria-label="t('workbench.startCard.start')" data-testid="dock-start-go" @click="start">
            <template #icon><ArrowUp :size="14" :stroke-width="1.9" /></template>
          </VButton>
        </div>
      </div>
      <p v-if="notice" class="foot" role="status" data-testid="dock-start-notice">{{ notice }}</p>
      <p v-else class="foot">{{ t('workbench.startCard.foot', { branch: target.worktree.branch }) }}</p>
    </div>
    <template v-else>
      <p class="hint">{{ t('workbench.pick') }}</p>
      <p v-if="keys.length" class="keys">{{ keys.join(' · ') }}</p>
    </template>
  </div>
</template>

<style scoped>
.start {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-3);
  height: 100%;
  padding: var(--vr-space-6);
}

.card-wrap {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: 100%;
  max-width: 640px;
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-xl);
  font-weight: 600;
  color: var(--vr-color-text);
}

.card {
  display: flex;
  flex-direction: column;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-2xl);
}

.files {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4) 0;
}

.prompt {
  width: 100%;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
  resize: none;
  background: transparent;
  border: 0;
  box-shadow: none;
  outline: none;
}

.bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}

.spacer {
  flex: 1;
}

.enter {
  color: var(--vr-color-text-subtle);
}

.foot {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.hint {
  margin: 0;
  font-size: var(--vr-font-size-title);
  color: var(--vr-color-text-muted);
}

.keys {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}
</style>
