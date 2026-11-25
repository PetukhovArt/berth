import { shallowRef } from 'vue';

const held = shallowRef<ReadonlyMap<string, readonly string[]>>(new Map());

export function holdRestored(agent: string, texts: readonly string[]): void {
  if (!texts.length) return;
  held.value = new Map(held.value).set(agent, [...(held.value.get(agent) ?? []), ...texts]);
}

export function restoredCount(agent: string): number {
  return held.value.get(agent)?.length ?? 0;
}

export function takeRestored(agent: string): string[] {
  const texts = held.value.get(agent);
  if (!texts) return [];
  const next = new Map(held.value);
  next.delete(agent);
  held.value = next;
  return [...texts];
}
