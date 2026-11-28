<script setup lang="ts">
import { vTooltip } from './kit';
import { computed } from 'vue';
import type { Person } from '../api/connectors/api';

const props = defineProps<{ person: Pick<Person, 'name' | 'username'> }>();

const label = computed(() => props.person.name || props.person.username);
</script>

<template>
  <span class="person-name" v-tooltip="label">{{ label }}</span>
</template>

<style scoped>
.person-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  word-break: keep-all;
}
</style>
