<script setup lang="ts">
import { MessageSquare } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { openChannelFor, useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';

withDefaults(defineProps<{ channelKey: string; variant?: 'button' | 'icon' }>(), { variant: 'button' });

const berth = useBerth();
</script>

<template>
  <template v-if="berth && channelKey">
    <VButton v-if="variant === 'button'" v-tooltip="t('attention.channel.openTitle', { key: channelKey })" size="small" severity="secondary" :label="t('attention.channel.open')" data-testid="open-channel" @click="openChannelFor(channelKey)">
      <template #icon><MessageSquare :size="14" /></template>
    </VButton>
    <VButton v-else v-tooltip="t('attention.channel.openTitle', { key: channelKey })" class="icon" text severity="secondary" size="small" :aria-label="t('attention.channel.open')" data-testid="open-channel" @click.stop="openChannelFor(channelKey)">
      <template #icon><MessageSquare :size="14" /></template>
    </VButton>
  </template>
</template>

<style scoped>
.icon {
  flex: none;
  width: 22px;
  height: 22px;
}
</style>
