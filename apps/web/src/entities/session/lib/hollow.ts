import { computed, type ComputedRef } from 'vue';
import { useBerth } from '../model/berth';

type Hollow ='connecting' | 'unknown';

export function useHollow(): ComputedRef<Hollow | null> {
  const berth = useBerth();
  return computed(() => {
    const state = berth?.connection.value.state;
    if (!state || state === 'connected') return null;
    return state === 'connecting' ? 'connecting' : 'unknown';
  });
}
