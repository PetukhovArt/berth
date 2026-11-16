<script setup lang="ts">
import { MessageSquare } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { openChannelFor, useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

withDefaults(defineProps<{ channelKey: string; variant?: 'button' | 'icon' }>(), { variant: 'button' });

const berth = useBerth();
</script>

<template>
  <template v-if="berth && channelKey">
    <VButton v-if="variant === 'button'" size="small" severity="secondary" :label="t('attention.channel.open')" :title="t('attention.channel.openTitle', { key: channelKey })" data-testid="open-channel" @click="openChannelFor(channelKey)">
      <template #icon><MessageSquare :size="14" /></template>
    </VButton>
    <UiButton v-else class="icon" type="button" :title="t('attention.channel.openTitle', { key: channelKey })" :aria-label="t('attention.channel.open')" data-testid="open-channel" @click.stop="openChannelFor(channelKey)">
      <MessageSquare :size="14" />
    </UiButton>
  </template>
</template>

<style scoped>
.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
