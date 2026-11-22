import type { DockviewApi } from 'dockview-vue';

const MIN_WIDTH = 168;
const MAX_WIDTH = 240;
const MENU_RESERVE = 56;
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

function fit(tabs: HTMLElement, bar: HTMLElement) {
  const count = tabs.querySelectorAll(':scope > .dv-tab').length;
  if (!count) return;
  const { other, menu } = measureBar(bar, tabs);
  const room = bar.clientWidth - other;
  let width: number;
  if (count * MIN_WIDTH <= room) {
    width = Math.min(MAX_WIDTH, Math.floor(room / count));
  } else {
    const usable = Math.max(MIN_WIDTH, room - menu);
    const visible = Math.max(1, Math.floor(usable / MIN_WIDTH));
    width = Math.min(MAX_WIDTH, Math.floor(usable / visible));
  }
  tabs.style.setProperty(WIDTH_VAR, `${width}px`);
}

export function trackTabFit(root: HTMLElement, api: DockviewApi): { dispose(): void }[] {
  let frame = 0;
  const resizes = new ResizeObserver(() => schedule());

  function run() {
    frame = 0;
    for (const tabs of root.querySelectorAll<HTMLElement>('.dv-tabs-container')) {
      const bar = tabs.closest<HTMLElement>('.dv-tabs-and-actions-container');
      if (!bar) continue;
      fit(tabs, bar);
      resizes.observe(bar);
      for (const child of Array.from(bar.children)) {
        if (!child.contains(tabs) && !child.classList.contains('dv-void-container')) resizes.observe(child);
      }
    }
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
