import type { DockviewApi, DockviewGroupPanel, DockviewPanelApi, IDockviewPanel } from 'dockview-vue';

const STORE_KEY = 'berth.dock.tabOrder';
const LIMIT = 500;
const DRAG_THRESHOLD = 5;

function load(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]');
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : [];
  } catch {
    return [];
  }
}

let order = load();

function save() {
  if (order.length > LIMIT) order = order.slice(-LIMIT);
  localStorage.setItem(STORE_KEY, JSON.stringify(order));
}

function forget(id: string) {
  const index = order.indexOf(id);
  if (index < 0) return;
  order.splice(index, 1);
  save();
}

function syncGroup(group: DockviewGroupPanel) {
  const ids = group.panels.map((panel) => panel.id);
  for (const id of ids) if (!order.includes(id)) order.push(id);
  const slots = ids.map((id) => order.indexOf(id)).sort((a, b) => a - b);
  ids.forEach((id, index) => {
    const slot = slots[index];
    if (slot !== undefined) order[slot] = id;
  });
  save();
}

const rank = (id: string) => {
  const index = order.indexOf(id);
  return index < 0 ? Number.POSITIVE_INFINITY : index;
};

function place(panel: IDockviewPanel) {
  if (!order.includes(panel.id)) {
    order.push(panel.id);
    save();
  }
  const group = panel.group;
  const target = group.panels.filter((other) => other !== panel && rank(other.id) < rank(panel.id)).length;
  if (group.panels.indexOf(panel) === target) return;
  const active = group.activePanel === panel;
  panel.api.moveTo({ group, index: target, skipSetActive: !active });
}

export function trackTabOrder(api: DockviewApi): { dispose(): void }[] {
  return [
    api.onDidAddPanel((panel) =>
      queueMicrotask(() => {
        if (api.getPanel(panel.id) === panel) place(panel);
      }),
    ),
    api.onDidRemovePanel((panel) =>
      queueMicrotask(() => {
        if (!api.getPanel(panel.id)) forget(panel.id);
      }),
    ),
    api.onDidMovePanel(({ panel }) => syncGroup(panel.group)),
  ];
}

export function startTabDrag(event: PointerEvent, panel: DockviewPanelApi, api: DockviewApi | null) {
  const host = event.currentTarget;
  if (event.button !== 0 || !api || !(host instanceof HTMLElement)) return;
  const source: HTMLElement = host;
  const startX = event.clientX;
  const startY = event.clientY;
  let dragging = false;
  let over: HTMLElement | null = null;
  let targetId: string | null = null;

  const mark = (el: HTMLElement | null, side?: 'before' | 'after') => {
    if (over && over !== el) delete over.dataset.dropSide;
    over = el;
    if (el && side) el.dataset.dropSide = side;
  };

  const indexOf = (id: string) => panel.group.panels.findIndex((other) => other.id === id);

  function onMove(move: PointerEvent) {
    if (!dragging && Math.hypot(move.clientX - startX, move.clientY - startY) < DRAG_THRESHOLD) return;
    if (!dragging) {
      dragging = true;
      source.dataset.dragging = 'true';
    }
    const hit = document.elementFromPoint(move.clientX, move.clientY);
    const el = hit?.closest('.dv-tab')?.querySelector<HTMLElement>('[data-panel-id]') ?? null;
    const id = el?.dataset.panelId;
    const target = id && id !== panel.id ? api?.getPanel(id) : undefined;
    if (!el || !id || !target || target.group !== panel.group) {
      targetId = null;
      mark(null);
      return;
    }
    targetId = id;
    mark(el, indexOf(id) > indexOf(panel.id) ? 'after' : 'before');
  }

  function finish(commit: boolean) {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    window.removeEventListener('pointercancel', onCancel);
    mark(null);
    delete source.dataset.dragging;
    if (!commit || !dragging || !targetId) return;
    const index = indexOf(targetId);
    if (index < 0) return;
    panel.moveTo({ group: panel.group, index });
    syncGroup(panel.group);
  }

  const onUp = () => finish(true);
  const onCancel = () => finish(false);
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
  window.addEventListener('pointercancel', onCancel);
}
