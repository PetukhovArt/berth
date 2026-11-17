export const hot = {
  title: 'Hot sessions',
  switcher: {
    dialog: 'Switch session',
    empty: 'No live sessions',
  },
  status: {
    feedback: 'Waiting for you',
    working: 'Working',
    idle: 'Idle',
    error: 'Error',
    limit: 'Limit',
  },
  overview: {
    title: 'Hot sessions overview',
    hint: 'Arrows to move, Enter to open, Esc to close · Alt+1–9 jump',
    close: 'Close overview',
    empty: 'No live sessions',
    noPreview: 'No activity yet',
  },
  mode: {
    titlebar: 'On',
    off: 'Off',
  },
  setting: {
    label: 'Hot sessions',
    description: 'Keep live sessions at hand: the {key:mod-shift-o} overview, {key:alt-1}..9 focuses the Nth one, {key:alt-j} jumps to the next one that needs you, {key:ctrl-tab} switches between recent ones.',
  },
} as const;
