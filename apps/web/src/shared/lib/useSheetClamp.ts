import { nextTick, watch, type Ref } from 'vue';

const MARGIN = 8;

function clamp(el: HTMLElement) {
  el.style.translate = '';
  const rect = el.getBoundingClientRect();
  const bounds = el.closest('main')?.getBoundingClientRect() ?? { left: 0, right: window.innerWidth };
  const left = bounds.left + MARGIN;
  const right = Math.max(left + rect.width, bounds.right - MARGIN);
  const shift = rect.left < left ? left - rect.left : rect.right > right ? right - rect.right : 0;
  if (shift) el.style.translate = `${shift}px 0`;
}

export function useSheetClamp(sheet: Ref<HTMLElement | null>, open: Ref<boolean>) {
  watch(open, async (value) => {
    if (!value) return;
    await nextTick();
    if (sheet.value) clamp(sheet.value);
    requestAnimationFrame(() => {
      if (open.value && sheet.value) clamp(sheet.value);
    });
  });
}
