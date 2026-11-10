import { shallowRef } from 'vue';
import type { ChangeRequestRef } from '../../connectors/api';

const shown = shallowRef<ReadonlyMap<string, ChangeRequestRef>>(new Map());

export const mrShownIn = (panelId: string): ChangeRequestRef | undefined => shown.value.get(panelId);

export function toggleMr(panelId: string, ref: ChangeRequestRef) {
  const next = new Map(shown.value);
  if (next.has(panelId)) next.delete(panelId);
  else next.set(panelId, ref);
  shown.value = next;
}
