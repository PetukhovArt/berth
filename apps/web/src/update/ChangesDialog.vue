<script setup lang="ts">
import { computed } from 'vue';
import { VButton, VDialog } from '@ui/kit';
import { t } from '../i18n';
import { changes, closeChanges, type ChangeKind } from './state';

const KINDS: ChangeKind[] = ['feat', 'perf', 'fix'];

const groups = computed(() =>
  KINDS.map((kind) => ({ kind, items: (changes.value ?? []).filter((change) => change.kind === kind).map((change) => change.text) })).filter((group) => group.items.length),
);

function onVisible(visible: boolean) {
  if (!visible) closeChanges();
}
</script>

<template>
  <VDialog :visible="changes !== null" modal :header="t('update.changes.title')" :style="{ width: '560px' }" :draggable="false" data-testid="update-changes" @update:visible="onVisible">
    <div class="body">
      <section v-for="group in groups" :key="group.kind" class="group" :data-testid="`update-changes-${group.kind}`">
        <h3 class="heading">{{ t(`update.changes.${group.kind}`) }}</h3>
        <ul class="list">
          <li v-for="(item, index) in group.items" :key="index">{{ item }}</li>
        </ul>
      </section>
    </div>
    <div class="actions">
      <VButton :label="t('update.changes.ok')" size="small" data-testid="update-changes-ok" @click="closeChanges" />
    </div>
  </VDialog>
</template>

<style scoped>
.body {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
  max-height: 60vh;
  margin-bottom: var(--vr-space-4);
  overflow: auto;
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.heading {
  margin: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin: 0;
  padding-left: var(--vr-space-4);
}

.list li {
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
