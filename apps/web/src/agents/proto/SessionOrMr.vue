<script setup lang="ts">
import { computed } from 'vue';
import ChangeRequestView from '../../forge/ChangeRequestView.vue';
import { mrShownIn } from './tabMrView';

const props = defineProps<{ panelId: string }>();

const mr = computed(() => mrShownIn(props.panelId));
</script>

<template>
  <div class="host" :data-view="mr ? 'mr' : 'session'">
    <div v-show="!mr" class="pane"><slot /></div>
    <ChangeRequestView v-if="mr" class="pane vr-fade-in" :connector="mr.connector" :project="mr.project" :number="mr.number" data-testid="session-mr-view" />
  </div>
</template>

<style scoped>
.host {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.pane {
  flex: 1;
  min-height: 0;
}
</style>
