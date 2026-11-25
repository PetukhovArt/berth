export function digitIndex(event: KeyboardEvent): number | null {
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || event.isComposing) return null;
  const target = event.target;
  if (target instanceof HTMLElement && (target.isContentEditable || /^(?:INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return null;
  const index = Number(event.key) - 1;
  return Number.isInteger(index) && index >= 0 ? index : null;
}
