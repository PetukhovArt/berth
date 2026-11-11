<script setup lang="ts">
import { Command, LayoutPanelLeft, Settings } from 'lucide-vue-next';
import { paletteOpen } from '../../../shared/lib/commands/context';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { navigate, view, type View } from '../../../shared/model/nav';
import { useDock } from '../../../shared/model/dock';

const { t } = useI18n();
const { detail } = useDock();

const ITEMS = [
  { kind: 'agents', label: 'shell.nav.agents', icon: LayoutPanelLeft },
  { kind: 'settings', label: 'shell.settings', icon: Settings },
] as const satisfies readonly { kind: View['kind']; label: string; icon: unknown }[];

const isActive = (kind: View['kind']) => view.value.kind === kind;

function go(kind: (typeof ITEMS)[number]['kind']) {
  if (kind === 'agents' && view.value.kind === 'agents') detail.value = false;
  else void navigate({ kind });
}
</script>

<template>
  <nav class="nav" :aria-label="t('mobile.navLabel')" data-testid="mobile-nav">
    <button v-for="item in ITEMS" :key="item.kind" class="item" :data-active="isActive(item.kind)" :data-testid="`mobile-nav-${item.kind}`" @click="go(item.kind)">
      <component :is="item.icon" :size="20" />
      <span>{{ t(item.label) }}</span>
    </button>
    <button class="item" :data-active="paletteOpen" data-testid="mobile-nav-commands" @click="paletteOpen = !paletteOpen">
      <Command :size="20" />
      <span>{{ t('mobile.commands') }}</span>
    </button>
  </nav>
</template>

<style scoped>
.nav {
  display: flex;
  flex: none;
  padding: var(--vr-space-1) var(--vr-space-1) calc(var(--vr-space-1) + var(--vr-safe-bottom));
  border-top: 1px solid var(--vr-color-border);
}

.item {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: var(--vr-space-1) 0;
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

@media (orientation: landscape) {
  .nav {
    flex-direction: column;
    justify-content: flex-start;
    gap: var(--vr-space-1);
    width: 4.5rem;
    padding: var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-1) + var(--vr-safe-bottom));
    border-top: 0;
    border-right: 1px solid var(--vr-color-border);
  }

  .item {
    flex: none;
    min-height: var(--vr-size-touch);
  }
}

.item[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}
</style>
