<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VDialog, VMessage, VSelect } from '../../../shared/ui/kit';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth, useAction, moveAgent, useWorktreeOptions } from '../../../entities/session';

const props = defineProps<{ agent: string }>();
const emit = defineEmits<{ moved: []; close: [] }>();

const berth = useBerth();
const selectPt = { label: { autofocus: true } };
const current = computed(() => berth?.entities.value.agents.get(props.agent)?.worktree_id);
const project = computed(() => (current.value ? berth?.entities.value.worktrees.get(current.value)?.project_id : undefined));
const options = useWorktreeOptions(() => project.value, () => current.value);

const picked = shallowRef<string | null>(null);
const { busy, error, run } = useAction();

const target = computed(() => options.value.find((option) => option.id === picked.value)?.id ?? options.value[0]?.id);

function pick(worktree: unknown) {
  if (typeof worktree === 'string') picked.value = worktree;
}

async function submit() {
  const worktree = target.value;
  if (!worktree) return;
  if (await run(() => moveAgent(props.agent, worktree))) emit('moved');
}
</script>

<template>
  <VDialog v-if="berth" :visible="true" modal :header="t('agents.move.title')" :style="{ width: '28rem', maxWidth: '95vw' }" data-testid="move-agent-dialog" @update:visible="emit('close')">
    <p v-if="!options.length" class="state" role="status">{{ t('agents.move.none') }}</p>
    <form v-else class="form" @submit.prevent="submit">
      <VSelect
        :model-value="target"
        :options="options"
        option-label="label"
        option-value="id"
        :aria-label="t('agents.move.target')"
        :pt="selectPt"
        data-testid="move-agent-worktree"
        @update:model-value="pick"
      />
      <VMessage v-if="error" severity="error" size="small" data-testid="move-agent-error">{{ error }}</VMessage>
      <DialogActions submit :confirm-label="t('agents.move.submit')" :busy="busy" :disabled="!target" focus="none" confirm-testid="move-agent-submit" @cancel="emit('close')" />
    </form>
    <DialogActions v-if="!options.length" class="actions" @cancel="emit('close')" />
  </VDialog>
</template>

<style scoped>
.state {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.actions {
  margin-top: var(--vr-space-3);
}
</style>
