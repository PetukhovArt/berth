import { computed, onBeforeUnmount, shallowRef, watch, type ShallowRef } from 'vue';

const TICK_MS = 1000;

export function useClock(needed: (now: number) => boolean): ShallowRef<number> {
  const now = shallowRef(Date.now());
  const active = computed(() => needed(now.value));
  let timer: ReturnType<typeof setInterval> | undefined;

  function stop() {
    clearInterval(timer);
    timer = undefined;
  }

  watch(
    active,
    (on) => {
      stop();
      if (!on) return;
      now.value = Date.now();
      timer = setInterval(() => (now.value = Date.now()), TICK_MS);
    },
    { immediate: true },
  );
  onBeforeUnmount(stop);
  return now;
}
