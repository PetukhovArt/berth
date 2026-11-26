<script setup lang="ts">
import { computed } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { badges, openCleanup } from '../model/state';
import { VButton, vTooltip } from '../../../shared/ui/kit';
const props = defineProps<{ worktreeId: string }>();

const kind = computed(() => badges.value.get(props.worktreeId) ?? null);
const label = computed(() => (kind.value ? t(`cleanup.badge.${kind.value}`) : ''));
</script>

<template>
  <VButton v-if="kind" v-tooltip="t('cleanup.badgeTitle', { reason: label })" class="badge" text severity="secondary" size="small" :label="label" :data-kind="kind" :aria-label="t('cleanup.badgeTitle', { reason: label })" data-testid="worktree-cleanup-badge" @click.stop="openCleanup" />
</template>

<style scoped>
.badge {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  line-height: 1.45;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.badge[data-kind='merged'] {
  color: var(--vr-color-success);
}

.badge[data-kind='closed'],
.badge[data-kind='gone'] {
  color: var(--vr-color-conflict);
}
</style>
