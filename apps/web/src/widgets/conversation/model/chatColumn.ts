import { computed, onBeforeUnmount, onMounted, shallowRef, type ShallowRef } from 'vue';
import { useIsPhone } from '../../../shared/ui/kit';

export type ChatColumn = { width: number; bias: number };
export type ColumnBox = { left: number; width: number };
export type ColumnEdge = 'left' | 'right';

export const COLUMN_EDGES: readonly ColumnEdge[] = ['left', 'right'];
const DEFAULT_COLUMN: ChatColumn = { width: 920, bias: 0.5 };
const MIN_COLUMN_WIDTH = 480;
export const COLUMN_GUTTER = 16;

const STORE_KEY = 'berth.chat.column';
const KEY_STEP = 16;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function storedColumn(): ChatColumn {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? 'null');
    if (typeof parsed !== 'object' || parsed === null) return DEFAULT_COLUMN;
    const { width, bias } = parsed as Record<string, unknown>;
    if (typeof width !== 'number' || typeof bias !== 'number') return DEFAULT_COLUMN;
    if (!Number.isFinite(width) || width < MIN_COLUMN_WIDTH || !Number.isFinite(bias) || bias < 0 || bias > 1) return DEFAULT_COLUMN;
    return { width, bias };
  } catch {
    return DEFAULT_COLUMN;
  }
}

const chatColumn = shallowRef<ChatColumn>(storedColumn());

function placeColumn(column: ChatColumn, track: number): ColumnBox {
  const min = Math.min(MIN_COLUMN_WIDTH, track);
  const width = Math.min(track, Math.max(min, column.width));
  const left = Math.round((track - width) * clamp(column.bias, 0, 1));
  return { left, width: Math.round(width) };
}

function moveEdge(box: ColumnBox, edge: ColumnEdge, dx: number, track: number): ChatColumn {
  const right = box.left + box.width;
  const min = Math.min(MIN_COLUMN_WIDTH, track);
  let left = box.left;
  let width = box.width;
  if (edge === 'left') {
    left = clamp(box.left + dx, 0, right - min);
    width = right - left;
  } else {
    width = clamp(right + dx, box.left + min, track) - left;
  }
  const free = track - width;
  return { width: Math.round(width), bias: free > 0 ? left / free : 0.5 };
}

function save() {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(chatColumn.value));
  } catch {
    return;
  }
}

export function useChatColumn(scroller: Readonly<ShallowRef<HTMLElement | null>>) {
  const track = shallowRef<number | null>(null);
  const dragging = shallowRef<ColumnEdge | null>(null);
  const phone = useIsPhone();
  let observer: ResizeObserver | undefined;

  onMounted(() => {
    const el = scroller.value;
    if (!el) return;
    observer = new ResizeObserver(() => {
      const next = el.clientWidth - 2 * COLUMN_GUTTER;
      if (next >= 1) track.value = next;
    });
    observer.observe(el);
  });

  onBeforeUnmount(() => observer?.disconnect());

  const box = computed(() => (track.value === null ? null : placeColumn(chatColumn.value, track.value)));
  const style = computed(() => {
    const current = box.value;
    if (!current || phone.value) return {};
    return { '--chat-column-left': `${COLUMN_GUTTER + current.left}px`, '--chat-column-width': `${current.width}px` };
  });

  function startDrag(edge: ColumnEdge, event: PointerEvent) {
    const handle = event.currentTarget;
    const startBox = box.value;
    if (!(handle instanceof HTMLElement) || !startBox) return;
    event.preventDefault();
    handle.focus();
    const startX = event.clientX;
    handle.setPointerCapture(event.pointerId);
    dragging.value = edge;
    const move = (next: PointerEvent) => {
      if (track.value !== null) chatColumn.value = moveEdge(startBox, edge, next.clientX - startX, track.value);
    };
    const end = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', end);
      handle.removeEventListener('pointercancel', end);
      dragging.value = null;
      save();
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', end);
    handle.addEventListener('pointercancel', end);
  }

  function nudge(edge: ColumnEdge, event: KeyboardEvent) {
    const step = event.key === 'ArrowLeft' ? -KEY_STEP : event.key === 'ArrowRight' ? KEY_STEP : 0;
    const current = box.value;
    if (!step || !current || track.value === null) return;
    event.preventDefault();
    chatColumn.value = moveEdge(current, edge, step, track.value);
    save();
  }

  function reset() {
    chatColumn.value = DEFAULT_COLUMN;
    try {
      localStorage.removeItem(STORE_KEY);
    } catch {
      return;
    }
  }

  return { track, box, style, dragging, phone, startDrag, nudge, reset };
}
