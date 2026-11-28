<script setup lang="ts">
import { vTooltip } from '../kit';
import { nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue';
import { ExternalLink, X } from '../icons';
import { t } from '../../lib/i18n/i18n';

const props = defineProps<{ src: string; alt: string; origin: string | null }>();
const emit = defineEmits<{ close: []; 'open-origin': [url: string] }>();

const actual = ref(false);
const closeButton = useTemplateRef<HTMLButtonElement>('closeButton');

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape') return;
  event.preventDefault();
  event.stopPropagation();
  emit('close');
}

const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;

onMounted(async () => {
  window.addEventListener('keydown', onKeydown, true);
  await nextTick();
  closeButton.value?.focus();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown, true);
  opener?.focus();
});
</script>

<template>
  <Teleport to="body">
    <div class="layer" role="dialog" aria-modal="true" :aria-label="props.alt || t('content.image.title')" data-testid="image-viewer" @click.self="emit('close')">
      <div class="viewer">
        <div class="bar">
          <span class="name">{{ props.alt || t('content.image.title') }}</span>
          <span class="hint">{{ actual ? t('content.image.fitHint') : t('content.image.actualHint') }}</span>
          <button v-if="props.origin" type="button" class="btn" data-testid="image-viewer-open" @click="emit('open-origin', props.origin)">
            <ExternalLink :size="14" />
            {{ t('content.image.openInBrowser') }}
          </button>
          <button ref="closeButton" type="button" class="btn icon" v-tooltip="t('content.mermaid.close')" :aria-label="t('content.mermaid.close')" data-testid="image-viewer-close" @click="emit('close')">
            <X :size="14" />
          </button>
        </div>
        <div class="stage" :class="{ actual }" @click.self="emit('close')">
          <img :src="props.src" :alt="props.alt" data-testid="image-viewer-img" @click="actual = !actual" />
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.layer {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  padding: var(--vr-space-4);
  background: var(--vr-palette-black-a-50);
  animation: vr-fade-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.viewer {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  background: var(--vr-color-surface-raised);
  box-shadow: var(--vr-shadow-popover);
  overflow: hidden;
  animation: vr-overlay-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.bar {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
  background: var(--vr-color-surface-muted);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.name {
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.hint {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.btn {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  height: 24px;
  padding: 0 var(--vr-space-2);
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: transparent;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-xs);
}

.btn.icon {
  width: 24px;
  padding: 0;
  justify-content: center;
}

.btn:hover {
  background: var(--vr-color-nav-active);
}

.stage {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  min-height: 0;
  padding: var(--vr-space-4);
  overflow: hidden;
}

.stage img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  cursor: zoom-in;
}

.stage.actual {
  align-items: flex-start;
  justify-content: flex-start;
  overflow: auto;
}

.stage.actual img {
  flex: none;
  max-width: none;
  max-height: none;
  margin: auto;
  cursor: zoom-out;
}
</style>
