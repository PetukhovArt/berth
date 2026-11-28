import type { DockviewApi } from 'dockview-vue';

const MIN_WIDTH = 168;
const MAX_WIDTH = 240;
const MENU_RESERVE = 56;
const SLACK = 2;
const FLOOR_WIDTH = 120;
const WIDTH_VAR = '--dock-tab-width';

const MENU = '.dv-tabs-overflow-dropdown-root';

function measureBar(bar: HTMLElement, tabs: HTMLElement): { other: number; menu: number } {
  let other = 0;
  let menu = 0;
  for (const child of Array.from(bar.children)) {
    if (!(child instanceof HTMLElement) || child.contains(tabs) || child.classList.contains('dv-void-container')) continue;
    const dropdown = child.matches(MENU) ? child : child.querySelector<HTMLElement>(MENU);
    if (!dropdown) {
      other += child.offsetWidth;
      continue;
    }
    menu = dropdown.offsetWidth;
    other += child === dropdown ? 0 : child.offsetWidth - menu;
  }
  return { other, menu: menu || MENU_RESERVE };
}

function fit(tabs: HTMLElement, bar: HTMLElement, clip: boolean): boolean {
  const count = tabs.querySelectorAll(':scope > .dv-tab').length;
  if (!count) return false;
  const { other, menu } = measureBar(bar, tabs);
  const room = bar.clientWidth - other - SLACK;
  let width: number;
  let shown = count;
  let stale = false;
  if (count * MIN_WIDTH <= room) {
    stale = clip && bar.querySelector(MENU) !== null;
    width = stale ? Math.floor((room - menu) / count) : Math.min(MAX_WIDTH, Math.floor(room / count));
  } else {
    const usable = room - menu;
    shown = Math.max(1, Math.floor(usable / MIN_WIDTH));
    width = Math.max(FLOOR_WIDTH, Math.min(MAX_WIDTH, Math.floor(usable / shown)));
  }
  tabs.style.setProperty(WIDTH_VAR, `${width}px`);
  if (clip) {
    tabs.style.maxWidth = `${shown * width}px`;
    snapScroll(tabs, width, shown, count);
  } else tabs.style.removeProperty('max-width');
  return stale;
}

function snapScroll(tabs: HTMLElement, width: number, shown: number, count: number) {
  const all = Array.from(tabs.querySelectorAll<HTMLElement>(':scope > .dv-tab'));
  const active = all.findIndex((tab) => tab.classList.contains('dv-active-tab'));
  let first = Math.round(tabs.scrollLeft / width);
  if (active >= 0 && active < first) first = active;
  if (active >= first + shown) first = active - shown + 1;
  first = Math.max(0, Math.min(first, count - shown));
  const target = first * width;
  if (Math.abs(tabs.scrollLeft - target) > 0.5) tabs.scrollLeft = target;
}

export function trackTabFit(root: HTMLElement, api: DockviewApi): { dispose(): void }[] {
  let frame = 0;
  const resizes = new ResizeObserver(() => schedule());

  function run() {
    frame = 0;
    let stale = false;
    for (const tabs of root.querySelectorAll<HTMLElement>('.dv-tabs-container')) {
      const bar = tabs.closest<HTMLElement>('.dv-tabs-and-actions-container');
      if (!bar) continue;
      if (fit(tabs, bar, root.dataset.tabs === 'scroll')) stale = true;
      resizes.observe(bar);
      for (const child of Array.from(bar.children)) {
        if (!child.contains(tabs) && !child.classList.contains('dv-void-container')) resizes.observe(child);
      }
    }
    if (stale) for (const group of api.groups) group.header.refreshOverflow();
  }

  function schedule() {
    if (!frame) frame = requestAnimationFrame(run);
  }

  schedule();
  return [
    api.onDidLayoutChange(schedule),
    api.onDidAddPanel(schedule),
    api.onDidRemovePanel(schedule),
    api.onDidMovePanel(schedule),
    api.onDidActivePanelChange(schedule),
    {
      dispose() {
        cancelAnimationFrame(frame);
        resizes.disconnect();
      },
    },
  ];
}
