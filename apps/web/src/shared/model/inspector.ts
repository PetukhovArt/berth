import { ref, shallowRef, watch } from 'vue';

const OPEN_KEY = 'berth.inspector.open';
const WIDTH_KEY = 'berth.inspector.width';

export const INSPECTOR_MIN_WIDTH = 240;
const INSPECTOR_DEFAULT_WIDTH = 300;

function readWidth(): number {
  const stored = Number(localStorage.getItem(WIDTH_KEY));
  return Number.isFinite(stored) && stored >= INSPECTOR_MIN_WIDTH ? Math.round(stored) : INSPECTOR_DEFAULT_WIDTH;
}

export const inspectorOpen = ref(localStorage.getItem(OPEN_KEY) !== '0');
export const inspectorWidth = ref(readWidth());
export const inspectorShown = shallowRef(false);

watch(inspectorOpen, (value) => localStorage.setItem(OPEN_KEY, value ? '1' : '0'));
watch(inspectorWidth, (value) => localStorage.setItem(WIDTH_KEY, String(value)));

export function toggleInspector(): void {
  inspectorOpen.value = !inspectorOpen.value;
}
