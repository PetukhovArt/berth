export type KeymapEntry = { context?: string; bindings: Record<string, string | null> };

export const DEFAULT_KEYMAP: readonly KeymapEntry[] = [
  {
    bindings: {
      'mod-shift-p': 'palette.open',
      f1: 'palette.open',
      'mod-alt-p': 'agents.quickOpen',
      'mod-shift-m': 'dock.toggleMaximize',
      'mod-shift-n': 'agents.newSession',
      'mod-shift-o': 'hot.overview',
      'ctrl-space t': 'session.view.terminal',
      'ctrl-space c': 'session.view.chat',
      'ctrl-space m': 'session.view.mr',
      'ctrl-space d': 'session.view.diff',
    },
  },
  {
    context: '!nativeMenu',
    bindings: {
      'mod-,': 'nav.settings',
    },
  },
  {
    context: 'terminalFocus',
    bindings: {
      'mod-=': 'terminal.zoomIn',
      'mod--': 'terminal.zoomOut',
      'mod-0': 'terminal.zoomReset',
    },
  },
  {
    context: '!terminalFocus && !editorFocus',
    bindings: {
      'mod-b': 'sidebar.toggle',
      'mod-k': 'sidebar.search',
    },
  },
  {
    context: '!terminalFocus',
    bindings: {
      'mod-p': 'agents.quickOpen',
      'mod-shift-r': 'nav.reviews',
      'mod-shift-j': 'nav.jira',
    },
  },
];
