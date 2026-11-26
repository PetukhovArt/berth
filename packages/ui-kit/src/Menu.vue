<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import Menu from 'primevue/menu';
import type { MenuItem } from 'primevue/menuitem';

type Item = MenuItem & { testid?: string; danger?: boolean };

const props = withDefaults(defineProps<{ model: Item[]; popup?: boolean; label?: string }>(), { popup: true, label: undefined });
const menu = useTemplateRef<InstanceType<typeof Menu>>('menu');

const items = computed(() => props.model.map((item) => (item.danger ? { ...item, class: [item.class, 'vr-menu-danger'] } : item)));

defineExpose({
  toggle: (event: Event) => menu.value?.toggle(event),
  show: (event: Event) => menu.value?.show(event),
  hide: () => menu.value?.hide(),
});
</script>

<template>
  <Menu ref="menu" class="vr-menu" :model="items" :popup="popup" :aria-label="label">
    <template #item="{ item, props: slot }">
      <a v-bind="slot.action" :data-testid="(item as Item).testid">
        <span v-if="item.icon" v-bind="slot.icon" />
        <span v-bind="slot.label">{{ item.label }}</span>
      </a>
    </template>
  </Menu>
</template>
