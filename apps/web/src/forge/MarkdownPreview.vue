<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { describeError, forgeFile, reloadWhenRecovered } from '../shared/api/connectors/api';
import RichContent from '../shared/ui/rich-content/RichContent.vue';
import { fsReadText } from '../shared/api/fs';
import { t } from '../shared/lib/i18n/i18n';
import { fileBaseUrl, localFilePath, type PreviewSource } from './fileBase';

const props = defineProps<{ source: PreviewSource; path: string }>();

const text = shallowRef<string | null>(null);
const failure = shallowRef<string | null>(null);

const localPath = computed(() => (props.source.kind === 'local' ? localFilePath(props.source.root, props.path) : null));
const basePath = computed(() => localPath.value?.replace(/[\\/][^\\/]*$/, ''));
const baseUrl = computed(() => (props.source.kind === 'forge' ? fileBaseUrl(props.source.requestUrl, props.source.sha, props.path) : undefined));
const connector = computed(() => (props.source.kind === 'forge' ? props.source.connector : undefined));

function read(): Promise<string> {
  const source = props.source;
  if (source.kind === 'local') return fsReadText(localFilePath(source.root, props.path));
  return forgeFile(source.connector, source.project, source.sha, props.path);
}

let generation = 0;

async function load() {
  const ticket = ++generation;
  text.value = null;
  failure.value = null;
  try {
    const content = await read();
    if (ticket === generation) text.value = content;
  } catch (e) {
    if (ticket === generation) failure.value = describeError(e);
  }
}

reloadWhenRecovered(
  () => connector.value ?? '',
  () => failure.value !== null && connector.value !== undefined,
  () => void load(),
);

watch(() => [props.source, props.path], load, { immediate: true });
</script>

<template>
  <div class="preview" data-testid="diff-md-preview">
    <p v-if="failure" class="note failure" role="alert">{{ failure }}</p>
    <p v-else-if="text === null" class="note">{{ t('forge.preview.loading') }}</p>
    <RichContent v-else :markdown="text" :connector="connector" :base-url="baseUrl" :base-path="basePath" variant="document" :breaks="false" />
  </div>
</template>

<style scoped>
.preview {
  padding: var(--vr-space-4) var(--vr-space-6);
}

.note {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.failure {
  color: var(--vr-color-danger);
}
</style>
