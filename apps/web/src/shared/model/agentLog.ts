import { shallowRef } from 'vue';

export const agentLogFor = shallowRef<string | null>(null);

export function toggleAgentLog(agentId: string) {
  agentLogFor.value = agentLogFor.value === agentId ? null : agentId;
}
