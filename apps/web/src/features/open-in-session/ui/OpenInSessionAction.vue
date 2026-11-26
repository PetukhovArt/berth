<script setup lang="ts">
import { computed } from 'vue';
import { LoaderCircle, SquareTerminal } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { useBerth } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { itemId } from '../../../entities/merge-request';
import { useOpenInSession, type MrItem } from '../model/openInSession';

const props = withDefaults(defineProps<{ item: MrItem; refs?: readonly string[]; variant?: 'button' | 'icon' }>(), { refs: () => [], variant: 'button' });

const berth = useBerth();
const { open, runs } = useOpenInSession();
const busy = computed(() => runs.value.get(itemId(props.item))?.busy ?? false);
</script>

<template>
  <template v-if="berth">
    <VButton v-if="variant === 'button'" size="small" severity="secondary" v-tooltip="t('forge.openSession.title')" :label="t('forge.openSession.action')" :loading="busy" data-testid="cr-open-session" @click="open(item, refs)">
      <template #icon><SquareTerminal :size="14" /></template>
    </VButton>
    <VButton v-else v-tooltip="t('forge.openSession.title')" class="icon" text severity="secondary" size="small" :aria-label="t('forge.openSession.action')" :disabled="busy" data-testid="mr-open-session" @click.stop="open(item, refs)">
      <template #icon>
        <LoaderCircle v-if="busy" :size="14" class="spin" />
        <SquareTerminal v-else :size="14" />
      </template>
    </VButton>
  </template>
</template>

<style scoped>
.icon {
  flex: none;
  width: 22px;
  height: 22px;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
