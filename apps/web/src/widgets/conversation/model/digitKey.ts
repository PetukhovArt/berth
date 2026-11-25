import { onBeforeUnmount, onMounted, type Ref } from 'vue';
import { useDock } from '../../../shared/model/dock';

function digitIndex(event: KeyboardEvent): number | null {
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || event.isComposing) return null;
  const target = event.target;
  if (target instanceof HTMLElement && (target.isContentEditable || /^(?:INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return null;
  const index = Number(event.key) - 1;
  return Number.isInteger(index) && index >= 0 ? index : null;
}

export function useDigitKeys(card: Ref<HTMLElement | null>, panelId: () => string | undefined, run: (index: number) => boolean) {
  const { activeKey } = useDock();

  function onKey(event: KeyboardEvent) {
    const index = digitIndex(event);
    const el = card.value;
    if (index === null || !el || !el.getClientRects().length) return;
    const id = panelId();
    if (id && activeKey.value !== id) return;
    if (run(index)) event.preventDefault();
  }

  onMounted(() => window.addEventListener('keydown', onKey));
  onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
}
