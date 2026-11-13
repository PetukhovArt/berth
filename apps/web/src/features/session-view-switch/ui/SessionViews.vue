<script setup lang="ts">
import { computed, nextTick, useTemplateRef, watch } from 'vue';
import { isAvailable, viewOf, viewProps, type SessionViewBinding, type SessionViewSlots } from '../model/sessionView';

const props = defineProps<{ panelId: string }>();
defineSlots<SessionViewSlots>();

const view = computed(() => viewOf(props.panelId));
const hasTerminal = computed(() => isAvailable(props.panelId, 'terminal'));
const shown = computed(() => (view.value === 'terminal' ? null : viewProps(props.panelId, view.value)));
const terminalPane = useTemplateRef<HTMLElement>('terminalPane');

watch(view, async (next, previous) => {
  if (next !== 'terminal' || previous === 'terminal') return;
  await nextTick();
  terminalPane.value?.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea')?.focus();
});
</script>

<template>
  <div class="host" :data-view="view === 'terminal' ? 'session' : view">
    <div v-if="hasTerminal" v-show="view === 'terminal'" ref="terminalPane" class="pane"><slot name="terminal" /></div>
    <slot v-if="shown" :name="view" v-bind="shown as SessionViewBinding" />
  </div>
</template>

<style scoped>
.host {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.pane,
:slotted(.pane) {
  flex: 1;
  min-height: 0;
}
</style>
