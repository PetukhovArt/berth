import { h, reactive, type VNode } from 'vue';

export const selectFilterTexts = reactive({ placeholder: '', count: '' });

export const FILTER_FROM_OPTIONS = 8;

export type LabelSource = string | ((option: unknown) => unknown) | undefined;

export function labelOf(option: unknown, source: LabelSource): string {
  if (typeof source === 'function') return String(source(option) ?? '');
  if (source && option && typeof option === 'object') return String((option as Record<string, unknown>)[source] ?? '');
  return String(option ?? '');
}

export function highlight(label: string, query: string): (string | VNode)[] {
  const at = query ? label.toLocaleLowerCase().indexOf(query.toLocaleLowerCase()) : -1;
  if (at < 0) return [label];
  const end = at + query.length;
  return [label.slice(0, at), h('span', { class: 'vr-select-match' }, label.slice(at, end)), label.slice(end)];
}
