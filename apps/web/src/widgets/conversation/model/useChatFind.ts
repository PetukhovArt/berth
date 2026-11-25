import { nextTick, onBeforeUnmount, shallowRef, watch, type Ref } from 'vue';
import { findOccurrences, stepFindIndex } from '../../../shared/lib/textFind';

const ALL = 'chat-find';
const CURRENT = 'chat-find-current';

function clearHighlights() {
  if (typeof CSS === 'undefined' || !CSS.highlights) return;
  CSS.highlights.delete(ALL);
  CSS.highlights.delete(CURRENT);
}

export function useChatFind(scroller: Ref<HTMLElement | null>, content: () => unknown) {
  const open = shallowRef(false);
  const query = shallowRef('');
  const index = shallowRef(0);
  const total = shallowRef(0);
  let ranges: Range[] = [];

  function scan() {
    ranges = [];
    const root = scroller.value;
    if (open.value && root && query.value) {
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        for (const at of findOccurrences(node.textContent ?? '', query.value)) {
          const range = document.createRange();
          range.setStart(node, at);
          range.setEnd(node, at + query.value.length);
          ranges.push(range);
        }
      }
    }
    total.value = ranges.length;
    index.value = Math.min(index.value, Math.max(ranges.length - 1, 0));
    paint(false);
  }

  function paint(reveal: boolean) {
    clearHighlights();
    if (!ranges.length || typeof Highlight === 'undefined') return;
    const current = ranges[index.value];
    CSS.highlights.set(ALL, new Highlight(...ranges));
    if (!current) return;
    CSS.highlights.set(CURRENT, new Highlight(current));
    if (reveal) current.startContainer.parentElement?.scrollIntoView({ block: 'center' });
  }

  function step(delta: number) {
    index.value = stepFindIndex(index.value, total.value, delta);
    paint(true);
  }

  async function show() {
    open.value = true;
    await nextTick();
    scan();
  }

  function hide() {
    open.value = false;
    query.value = '';
    index.value = 0;
    scan();
  }

  watch(query, () => {
    index.value = 0;
    scan();
    paint(true);
  });
  watch(content, () => open.value && void nextTick(scan), { flush: 'post' });
  onBeforeUnmount(clearHighlights);

  return { open, query, index, total, step, show, hide };
}
