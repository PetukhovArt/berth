import { onActivated, onBeforeUnmount, onDeactivated, shallowRef } from 'vue';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-vue';
import { useIsPhone } from '../ui/kit';
import { navigate, view } from './nav';

export type PanelSpec = { id: string; component: string; title: string; params?: Record<string, unknown>; renderer?: 'always' };
export type PanelPosition = { referencePanel: string; direction: 'within' | 'right' | 'below' | 'left' };
type DockHooks = {
  keep?: (id: string) => boolean;
  ready?: (api: DockviewApi) => void;
  change?: (api: DockviewApi) => void;
};
type Reviver = (id: string) => PanelSpec | undefined;
type Carried ={ id: string; component: string; title: string; params: Record<string, unknown>; renderer?: 'always' };

export function splitPosition(api: DockviewApi): PanelPosition | undefined {
  return api.activePanel ? { referencePanel: api.activePanel.id, direction: 'right' } : undefined;
}

function createDock() {
  const mobile = useIsPhone();
  const api = shallowRef<DockviewApi | null>(null);
  const activeKey = shallowRef<string | null>(null);
  const activeTitle = shallowRef('');
  const visible = shallowRef(false);
  const detail = shallowRef(false);
  const switching = shallowRef(false);
  const pending: ((api: DockviewApi) => void)[] = [];
  const hooks: DockHooks[] = [];
  const revivers = new Set<Reviver>();
  let carried: { panels: Carried[]; active: string | null } = { panels: [], active: null };

  function hook(next: DockHooks) {
    hooks.push(next);
  }

  function revivable(reviver: Reviver) {
    revivers.add(reviver);
  }

  function revive(id: string): PanelSpec | undefined {
    for (const reviver of revivers) {
      const panel = reviver(id);
      if (panel) return panel;
    }
    return undefined;
  }

  function changed(dock: DockviewApi) {
    for (const each of hooks) each.change?.(dock);
  }

  function flush() {
    const dock = api.value;
    if (!dock || !visible.value) return;
    for (const run of pending.splice(0)) run(dock);
  }

  function withDock(run: (api: DockviewApi) => void, reveal = true) {
    if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
    if (reveal && mobile.value) detail.value = true;
    pending.push(run);
    flush();
  }

  function openPanel(panel: PanelSpec, at: boolean | PanelPosition = false) {
    withDock((dock) => {
      const existing = dock.getPanel(panel.id);
      const position = at === true ? splitPosition(dock) : at || undefined;
      if (existing) existing.api.setActive();
      else dock.addPanel(position ? { ...panel, position } : panel);
    });
  }

  function toggleMaximize(panelId?: string) {
    const dock = api.value;
    if (!dock) return;
    if (dock.hasMaximizedGroup()) {
      dock.exitMaximizedGroup();
      return;
    }
    const panel = panelId ? dock.getPanel(panelId) : dock.activePanel;
    if (panel && dock.groups.length > 1) panel.api.maximize();
  }

  function restore(dock: DockviewApi) {
    for (const panel of carried.panels) {
      if (!hooks.every((each) => each.keep?.(panel.id) ?? true)) continue;
      dock.addPanel({ id: panel.id, component: panel.component, title: panel.title, params: panel.params, ...(panel.renderer ? { renderer: panel.renderer } : {}) });
    }
    if (carried.active) dock.getPanel(carried.active)?.api.setActive();
    carried = { panels: [], active: null };
  }

  function useHost() {
    let listeners: { dispose(): void }[] = [];
    visible.value = true;

    function onReady({ api: dock }: DockviewReadyEvent) {
      api.value = dock;
      listeners = [
        dock.onDidActivePanelChange(({ panel }) => {
          activeKey.value = panel?.id ?? null;
          activeTitle.value = panel?.title ?? '';
          changed(dock);
        }),
        dock.onDidAddPanel(() => changed(dock)),
        dock.onDidRemovePanel(() => changed(dock)),
      ];
      const queued = pending.splice(0);
      restore(dock);
      for (const each of hooks) each.ready?.(dock);
      pending.push(...queued);
      flush();
    }

    onActivated(() => {
      visible.value = true;
      flush();
    });
    onDeactivated(() => {
      visible.value = false;
    });
    onBeforeUnmount(() => {
      const dock = api.value;
      if (dock)
        carried = {
          panels: dock.panels.map((panel) => ({
            id: panel.id,
            component: panel.view.contentComponent,
            title: panel.title ?? panel.id,
            params: { ...panel.params },
            ...(panel.api.renderer === 'always' ? { renderer: 'always' as const } : {}),
          })),
          active: activeKey.value,
        };
      for (const listener of listeners) listener.dispose();
      api.value = null;
      visible.value = false;
    });

    return { onReady };
  }

  return { api, activeKey, activeTitle, visible, detail, switching, hook, revivable, revive, withDock, openPanel, toggleMaximize, useHost };
}

let store: ReturnType<typeof createDock> | null = null;

export function useDock() {
  store ??= createDock();
  return store;
}

export function toggleMaximizedGroup(): void {
  useDock().toggleMaximize();
}

export function toggleQuickOpen(): void {
  const { switching } = useDock();
  switching.value = !switching.value;
}
