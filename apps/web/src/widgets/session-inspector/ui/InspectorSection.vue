<script setup lang="ts">
defineProps<{ title: string; meta?: string; testid: string }>();
</script>

<template>
  <section class="section" :aria-label="title" :data-testid="testid">
    <header class="head">
      <h3 class="title">{{ title }}</h3>
      <span v-if="meta" class="meta">{{ meta }}</span>
      <slot name="action" />
    </header>
    <slot />
  </section>
</template>

<style scoped>
.section {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: var(--vr-space-3) 14px;
  border-bottom: 1px solid var(--vr-color-border);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: 20px;
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  font-variant-numeric: tabular-nums;
}

.section :deep(.row) {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: 22px;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
}

.section :deep(.row > .icon) {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.section :deep(.row > .grow) {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.section :deep(.row > .end) {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.section :deep(.note) {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.section :deep(.error) {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-status-crashed);
}

.section :deep(.link) {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-link);
  background: none;
  border: 0;
  cursor: pointer;
}

.section :deep(.link:hover),
.section :deep(.link:focus-visible) {
  text-decoration: underline;
}
</style>
