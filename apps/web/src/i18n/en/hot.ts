export const hot = {
  title: 'Hot sessions',
  bar: {
    label: 'Hot sessions',
    pin: 'Pin',
    unpin: 'Unpin',
    empty: 'No live sessions',
    more: '{count} more sessions',
  },
  switcher: {
    dialog: 'Switch session',
    empty: 'No live sessions',
  },
  status: {
    waiting: 'Waiting for you',
    working: 'Working',
    idle: 'Idle',
    error: 'Error',
  },
  overview: {
    title: 'Hot sessions overview',
    hint: 'Arrows to move, Enter to open, Esc to close',
    open: 'Hot sessions overview ({key:mod-shift-o})',
    close: 'Close overview',
    empty: 'No live sessions',
    noPreview: 'No activity yet',
  },
  toast: {
    waiting: '{count} waiting',
    nobody: 'Nobody is waiting',
  },
  mode: {
    titlebar: 'In the title bar',
    off: 'Off',
  },
  setting: {
    label: 'Hot sessions',
    description: 'Keep live sessions at hand: chips in the title bar and the {key:mod-shift-o} overview. {key:alt-1}..9 focuses the Nth one, {key:alt-j} jumps to the next one waiting for you, {key:ctrl-tab} switches between recent ones.',
  },
} as const;
