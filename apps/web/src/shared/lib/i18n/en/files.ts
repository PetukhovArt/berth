export const files = {
  markdown: {
    viewMode: 'View mode',
    code: 'Code',
    split: 'Split',
    preview: 'Preview',
    toc: 'Contents',
    openExternalTitle: 'Open in the default application',
    openExternal: 'Open in editor',
  },
  editor: {
    dirty: 'Unsaved changes',
    saveTitle: 'Save ({key:mod-s})',
    changed: {
      title: 'Changed on disk',
      hint: 'This file was modified outside Berth while you have unsaved edits.',
      reload: 'Reload',
      keep: 'Keep mine (overwrite)',
      compare: 'Compare',
    },
    compare: {
      disk: 'On disk',
      mine: 'Your version',
    },
    unsaved: {
      title: 'Unsaved changes',
      message: 'Unsaved changes in {names}. Save them before closing?',
      discard: 'Discard',
    },
  },
  settings: {
    category: 'Files',
    autoSave: {
      label: 'Auto-save Markdown files',
      description: 'Off: save with {key:mod-s}. On blur: save when the editor loses focus. After delay: save one second after the last edit.',
      off: 'Off',
      onBlur: 'On blur',
      afterDelay: 'After delay',
    },
  },
  picker: {
    open: 'Open a Markdown file',
    filter: 'Filter Markdown files',
    none: 'No Markdown files in this worktree',
  },
  switcher: {
    hint: 'Enter opens, Ctrl+Enter shows all results',
  },
} as const;
