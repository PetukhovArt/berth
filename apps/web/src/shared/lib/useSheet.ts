import { nextTick, watch, type Ref } from 'vue';

const MARGIN = 8;
const OWN_OVERLAYS = '.p-select-overlay';

function clamp(el: HTMLElement) {
  el.style.translate = '';
  if (getComputedStyle(el).position === 'fixed') return;
  const rect = el.getBoundingClientRect();
  const bounds = el.closest('main')?.getBoundingClientRect() ?? { left: 0, right: window.innerWidth };
  const left = bounds.left + MARGIN;
  const right = Math.max(left + rect.width, bounds.right - MARGIN);
  const shift = rect.left < left ? left - rect.left : rect.right > right ? right - rect.right : 0;
  if (shift) el.style.translate = `${shift}px 0`;
}

export function useSheet(sheet: Ref<HTMLElement | null>, open: Ref<boolean>) {
  watch(open, async (value, _, onCleanup) => {
    if (!value) return;
    const dismiss = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Element && (sheet.value?.parentElement?.contains(target) || target.closest(OWN_OVERLAYS))) return;
      open.value = false;
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') open.value = false;
    };
    document.addEventListener('pointerdown', dismiss, true);
    document.addEventListener('keydown', escape);
    onCleanup(() => {
      document.removeEventListener('pointerdown', dismiss, true);
      document.removeEventListener('keydown', escape);
    });
    await nextTick();
    if (sheet.value) clamp(sheet.value);
    requestAnimationFrame(() => {
      if (open.value && sheet.value) clamp(sheet.value);
    });
  });
}
