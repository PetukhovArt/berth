<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { VButton, VListbox, VMessage, VTag, isMotionEnabled, vTooltip } from '../../../shared/ui/kit';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { confluencePage, describeError, openUrl, reloadWhenRecovered, type ConfluencePage } from '../../../shared/api/connectors/api';
import Skeleton from '../../../shared/ui/Skeleton.vue';
import { rememberPage } from '../../../shared/lib/atlassian';

type Heading = { index: number; text: string; level: number };

const props = defineProps<{ connector: string; id: string }>();
const emit = defineEmits<{ open: [payload: { connector: string; id: string }] }>();

const page = ref<ConfluencePage | null>(null);
const loading = ref(false);
const error = ref('');
const headings = ref<Heading[]>([]);
const activeIndex = ref(0);
const scroller = ref<HTMLElement | null>(null);
const bodyEl = ref<HTMLElement | null>(null);
let elements: HTMLElement[] = [];
let observer: MutationObserver | undefined;
let frame = 0;
let seq = 0;

async function load() {
  const id = ++seq;
  loading.value = true;
  error.value = '';
  page.value = null;
  headings.value = [];
  elements = [];
  try {
    const result = await confluencePage(props.connector, props.id);
    if (id !== seq) return;
    page.value = result;
    rememberPage({ connector: result.connector, id: result.id, title: result.title, space: result.space });
  } catch (e) {
    if (id === seq) error.value = describeError(e);
  } finally {
    if (id === seq) loading.value = false;
  }
}

function collect() {
  const found = bodyEl.value ? Array.from(bodyEl.value.querySelectorAll<HTMLElement>('h1, h2, h3, h4')).filter((el) => el.textContent?.trim()) : [];
  elements = found;
  headings.value = found.map((el, index) => ({ index, text: el.textContent!.trim(), level: Number(el.tagName[1]) }));
  spy();
}

function observe() {
  observer?.disconnect();
  if (!bodyEl.value) return;
  observer = new MutationObserver(() => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(collect);
  });
  observer.observe(bodyEl.value, { childList: true, subtree: true });
  collect();
}

function spy() {
  const top = scroller.value?.getBoundingClientRect().top ?? 0;
  let active = 0;
  elements.forEach((el, index) => {
    if (el.getBoundingClientRect().top - top <= 24) active = index;
  });
  activeIndex.value = active;
}

const tocOptions = computed(() => headings.value.map((heading) => ({ value: String(heading.index), label: heading.text })));

function jumpTo(value: string | string[] | null | undefined) {
  if (typeof value === 'string') jump(Number(value));
}

function jump(index: number) {
  elements[index]?.scrollIntoView({ behavior: isMotionEnabled() ? 'smooth' : 'instant', block: 'start' });
  activeIndex.value = index;
}

watch(bodyEl, (el) => (el ? observe() : observer?.disconnect()), { flush: 'post' });
reloadWhenRecovered(
  () => props.connector,
  () => error.value !== '',
  () => void load(),
);

watch(() => [props.connector, props.id], load, { immediate: true });
onBeforeUnmount(() => {
  observer?.disconnect();
  cancelAnimationFrame(frame);
});
</script>

<template>
  <div ref="scroller" class="page" data-testid="confluence-page" @scroll.passive="spy">
    <VMessage v-if="error" class="error" severity="error" size="small" data-testid="confluence-page-error">
      <span class="error-row">
        <span class="error-text">{{ error }}</span>
        <VButton :label="t('common.retry')" size="small" severity="secondary" @click="load" />
      </span>
    </VMessage>
    <Skeleton v-else-if="loading" :rows="12" />
    <template v-else-if="page">
      <header class="head">
        <nav v-if="page.ancestors.length" class="crumbs" :aria-label="t('confluence.page.path')">
          <template v-for="ancestor in page.ancestors" :key="ancestor.id">
            <VButton :label="ancestor.title" size="small" text severity="secondary" @click="emit('open', { connector: connector, id: ancestor.id })" />
            <span class="sep">/</span>
          </template>
        </nav>
        <h1 class="title">{{ page.title }}</h1>
        <div class="meta">
          <VTag :value="page.space" severity="secondary" />
          <span class="muted">{{ t('confluence.page.version', { version: page.version }) }}</span>
          <span class="spacer" />
          <VButton :label="t('common.openInBrowser')" size="small" severity="secondary" @click="openUrl(page.url)" />
        </div>
      </header>
      <div class="layout">
        <div ref="bodyEl" class="content">
          <RichContent :html="page.body_html" :connector="page.connector" />
        </div>
        <nav v-if="headings.length" class="toc vr-wide-only" :aria-label="t('confluence.page.contents')">
          <div class="toc-title">{{ t('confluence.page.contents') }}</div>
          <VListbox flat :model-value="String(activeIndex)" :options="tocOptions" :label="t('confluence.page.contents')" :filter="false" test-id="confluence-toc" @update:model-value="jumpTo">
            <template #option="{ option }">
              <span v-tooltip.left="option.label" class="toc-item" :style="{ paddingLeft: `${((headings[Number(option.value)]?.level ?? 1) - 1) * 12}px` }">{{ option.label }}</span>
            </template>
          </VListbox>
        </nav>
      </div>
    </template>
  </div>
</template>

<style scoped>
.page {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: auto;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.head {
  padding: var(--vr-space-4) var(--vr-space-6) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.crumbs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
}

.error {
  margin: var(--vr-space-4);
}

.error-row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  width: 100%;
}

.error-text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}

.sep {
  color: var(--vr-color-text-muted);
}

.title {
  margin: 0 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xl);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.meta {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}

.muted {
  color: var(--vr-color-text-muted);
}

.spacer {
  flex: 1;
}

.layout {
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-6);
  padding: var(--vr-space-4) var(--vr-page-gutter);
}

.content {
  flex: 1;
  min-width: 0;
}

.toc {
  position: sticky;
  top: var(--vr-space-4);
  display: flex;
  flex: none;
  flex-direction: column;
  width: 220px;
  max-height: calc(100vh - 160px);
  overflow: auto;
  border-left: 1px solid var(--vr-color-border);
}

.toc-title {
  padding: 0 var(--vr-space-2) var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  text-transform: uppercase;
}

.toc-item {
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
