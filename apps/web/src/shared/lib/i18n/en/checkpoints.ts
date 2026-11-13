export const checkpoints = {
  open: 'Checkpoints',
  title: 'Checkpoints of {name}',
  hint: 'The files of this worktree as they were at each prompt. The branch is never touched.',
  empty: 'A checkpoint appears at the next prompt.',
  entry: 'Checkpoint {number}',
  beforeRestore: 'Before restore {number}',
  restore: 'Restore files',
  confirm: 'Replace the files of this worktree with Checkpoint {number}? What is there now is saved as a new Checkpoint first.',
  confirmRestore: 'Restore',
  cancel: 'Cancel',
} as const;
