import type { DockviewApi } from 'dockview-vue';

const MIN_WIDTH = 168;
const MAX_WIDTH = 240;
const MENU_RESERVE = 56;
const SLACK = 2;
const FLOOR_WIDTH = 120;
const WHEEL_STEP = 40;
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

const lastFit = new WeakMap<HTMLElement, { active: number; width: number; count: number }>();

function snapScroll(tabs: HTMLElement, width: number, shown: number, count: number) {
  const all = Array.from(tabs.querySelectorAll<HTMLElement>(':scope > .dv-tab'));
  const active = all.findIndex((tab) => tab.classList.contains('dv-active-tab'));
  const prev = lastFit.get(tabs);
  const reveal = !prev || prev.active !== active || prev.width !== width || prev.count !== count;
  lastFit.set(tabs, { active, width, count });
  let first = Math.round(tabs.scrollLeft / width);
  if (reveal && active >= 0 && active < first) first = active;
  if (reveal && active >= first + shown) first = active - shown + 1;
  first = Math.max(0, Math.min(first, count - shown));
  const target = first * width;
  if (Math.abs(tabs.scrollLeft - target) > 0.5) tabs.scrollLeft = target;
}

export function trackTabFit(root: HTMLElement, api: DockviewApi): { dispose(): void }[] {
  let frame = 0;
  const resizes = new ResizeObserver(() => schedule());
  const wheeled = new Set<HTMLElement>();
  let pending = 0;

  function stepByTab(event: WheelEvent) {
    if (root.dataset.tabs !== 'scroll') return;
    const tabs = (event.currentTarget as HTMLElement).querySelector<HTMLElement>(':scope > .dv-tabs-container');
    const width = tabs ? parseFloat(tabs.style.getPropertyValue(WIDTH_VAR)) : 0;
    if (!tabs || !width || tabs.scrollWidth <= tabs.clientWidth) return;
    event.preventDefault();
    event.stopPropagation();
    pending += Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
    if (Math.abs(pending) < WHEEL_STEP) return;
    const step = Math.sign(pending);
    pending = 0;
    const last = Math.round((tabs.scrollWidth - tabs.clientWidth) / width);
    const next = Math.max(0, Math.min(last, Math.round(tabs.scrollLeft / width) + step));
    tabs.scrollLeft = next * width;
  }

  function run() {
    frame = 0;
    let stale = false;
    for (const tabs of root.querySelectorAll<HTMLElement>('.dv-tabs-container')) {
      const bar = tabs.closest<HTMLElement>('.dv-tabs-and-actions-container');
      if (!bar) continue;
      if (fit(tabs, bar, root.dataset.tabs === 'scroll')) stale = true;
      const scroller = tabs.closest<HTMLElement>('.dv-scrollable');
      if (scroller && !wheeled.has(scroller)) {
        wheeled.add(scroller);
        scroller.addEventListener('wheel', stepByTab, { capture: true, passive: false });
      }
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
        for (const scroller of wheeled) scroller.removeEventListener('wheel', stepByTab, { capture: true });
        wheeled.clear();
      },
    },
  ];
}
