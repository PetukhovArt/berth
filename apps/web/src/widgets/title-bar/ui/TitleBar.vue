<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { Copy, Minus, Square, X } from '../../../shared/ui/icons';
import { hostWindow as win } from '../../../bridge';
import { settleAll } from '../../../entities/document';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { hostPlatform } from '../../../shared/lib/platform';
import { UpdateChip } from '../../../features/update';

import { UiButton } from '../../../shared/ui/UiButton';
const { t } = useI18n();
const maximized = ref(false);
const nativeControls = computed(() => hostPlatform.value.windowChrome.nativeDecorations);
let stopResize: (() => void) | undefined;

async function closeWindow() {
  if (await settleAll()) await win.close();
}

async function sync() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  await sync();
  stopResize = await win.onResized(() => void sync());
});

onUnmounted(() => stopResize?.());
</script>

<template>
  <header class="titlebar" data-tauri-drag-region data-testid="titlebar" :data-native-controls="nativeControls">
    <svg class="mark" data-tauri-drag-region viewBox="0 0 96 96" width="16" height="16" aria-hidden="true">
      <path class="stroke" d="M32 8 V60 C32 76 42 86 54 86 C68 86 80 76 80 62 C80 48 68 38 54 38 C44 38 34 44 18 60" />
      <circle class="node" cx="55" cy="62" r="6.5" />
    </svg>
    <div class="update" data-tauri-drag-region><UpdateChip /></div>
    <div v-if="!nativeControls" class="controls">
      <UiButton type="button" class="ctl" data-testid="titlebar-minimize" :aria-label="t('shell.window.minimize')" :title="t('shell.window.minimize')" @click="win.minimize()">
        <Minus :size="16" :stroke-width="1.5" />
      </UiButton>
      <UiButton
        type="button"
        class="ctl"
        data-testid="titlebar-maximize"
        :data-maximized="maximized"
        :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        :title="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        @click="win.toggleMaximize()"
      >
        <Copy v-if="maximized" :size="13" :stroke-width="1.5" />
        <Square v-else :size="12" :stroke-width="1.5" />
      </UiButton>
      <UiButton type="button" class="ctl close" data-testid="titlebar-close" :aria-label="t('shell.window.close')" :title="t('shell.window.close')" @click="closeWindow">
        <X :size="16" :stroke-width="1.5" />
      </UiButton>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 32px;
  padding-left: var(--vr-space-3);
  user-select: none;
  -webkit-user-select: none;
}

@media (pointer: coarse) {
  :root[data-vr-viewport='phone'] .titlebar {
    display: none;
  }
}

.titlebar[data-native-controls='true'] {
  padding-left: 78px;
}

.mark {
  flex: none;
  fill: none;
}

.stroke {
  stroke: var(--vr-color-text);
  stroke-width: 10;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.node {
  fill: var(--vr-color-accent);
}

.update {
  display: flex;
  flex: 1;
  justify-content: flex-end;
  padding-right: var(--vr-space-2);
}

.controls {
  display: flex;
  height: 100%;
}

.ctl {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  padding: 0;
  color: var(--vr-color-text);
  background: transparent;
  border: 0;
  border-radius: 0;
}

.ctl:hover {
  background: var(--vr-color-nav-hover);
}

.ctl:active {
  background: var(--vr-color-nav-active);
}

.ctl.close:hover {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}
</style>
