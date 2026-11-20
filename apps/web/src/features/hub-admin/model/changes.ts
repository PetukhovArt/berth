import { shallowRef } from 'vue';
import { changeHub, describeError, type HubChange } from '../../../entities/session';

export function useChanges() {
  const busy = shallowRef<ReadonlySet<string>>(new Set());
  const errors = shallowRef<ReadonlyMap<string, string>>(new Map());

  function setError(key: string, message: string | null) {
    const next = new Map(errors.value);
    if (message === null) next.delete(key);
    else next.set(key, message);
    errors.value = next;
  }

  function setBusy(key: string, on: boolean) {
    const next = new Set(busy.value);
    if (on) next.add(key);
    else next.delete(key);
    busy.value = next;
  }

  async function run(key: string, change: HubChange): Promise<boolean> {
    if (busy.value.has(key)) return false;
    setBusy(key, true);
    setError(key, null);
    try {
      await changeHub(change);
      return true;
    } catch (failure) {
      setError(key, describeError(failure));
      return false;
    } finally {
      setBusy(key, false);
    }
  }

  return { busy, errors, run };
}
