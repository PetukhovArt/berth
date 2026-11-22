import { nextTick, shallowRef, watch, type Ref, type ShallowRef } from 'vue';

type PickerOptions<T> = {
  results: Readonly<Ref<readonly T[]>>;
  query: Readonly<Ref<string>>;
  scope?: Readonly<Ref<string>>;
  idOf?: (item: T) => string;
  list: Readonly<ShallowRef<HTMLElement | null>>;
  choose: (item: T | undefined) => void;
  close: () => void;
};

export function usePickerList<T>(options: PickerOptions<T>): { index: ShallowRef<number>; onKey: (event: KeyboardEvent) => void } {
  const index = shallowRef(0);

  watch(
    [() => options.results.value, () => options.query.value, () => options.scope?.value],
    ([next, query, scope], [previous, previousQuery, previousScope]) => {
      if (query !== previousQuery || scope !== previousScope) {
        index.value = 0;
        return;
      }
      const { idOf } = options;
      const held = idOf ? previous[index.value] : undefined;
      const moved = held && idOf ? next.findIndex((item) => idOf(item) === idOf(held)) : -1;
      index.value = moved >= 0 ? moved : Math.min(index.value, Math.max(next.length - 1, 0));
    },
  );

  function move(delta: number) {
    const count = options.results.value.length;
    if (!count) return;
    index.value = (index.value + delta + count) % count;
    void nextTick(() => options.list.value?.querySelector('[data-current="true"]')?.scrollIntoView({ block: 'nearest' }));
  }

  function onKey(event: KeyboardEvent) {
    switch (event.key) {
      case 'ArrowDown':
        move(1);
        break;
      case 'ArrowUp':
        move(-1);
        break;
      case 'Enter':
        options.choose(options.results.value[index.value]);
        break;
      case 'Escape':
        options.close();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  return { index, onKey };
}
