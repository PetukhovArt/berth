import { computed, shallowRef, watch, type ComputedRef } from 'vue';
import { readTranscript, useExternalSessions } from './external';
import { sessionKey, type SessionRef } from './berth';
import { derivePlan, type PlanItem } from './plan';

const TAIL = 400;

export function useLinkedPlan(ref: () => SessionRef): ComputedRef<PlanItem[]> {
  const external = useExternalSessions();
  const linked = computed(() => external?.linked.value.get(sessionKey(ref())));
  const plan = shallowRef<PlanItem[]>([]);
  let generation = 0;

  watch(
    () => [linked.value?.harness, linked.value?.id, linked.value?.updated_at] as const,
    async ([harness, id]) => {
      const current = ++generation;
      if (!harness || !id) {
        plan.value = [];
        return;
      }
      try {
        const messages = await readTranscript({ harness, id }, TAIL);
        if (current === generation) plan.value = derivePlan(messages);
      } catch {
        if (current === generation) plan.value = [];
      }
    },
    { immediate: true },
  );

  return computed(() => plan.value);
}
