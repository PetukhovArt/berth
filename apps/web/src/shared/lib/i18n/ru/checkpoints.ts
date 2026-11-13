export const checkpoints = {
  open: 'Чекпоинты',
  title: 'Чекпоинты {name}',
  hint: 'Файлы этого worktree на момент каждого промпта. Ветка не затрагивается.',
  empty: 'Чекпоинт появится при следующем промпте.',
  entry: 'Чекпоинт {number}',
  beforeRestore: 'Перед восстановлением {number}',
  restore: 'Восстановить файлы',
  confirm: 'Заменить файлы этого worktree на Чекпоинт {number}? То, что там сейчас, сначала сохранится новым Чекпоинтом.',
  confirmRestore: 'Восстановить',
  cancel: 'Отмена',
} as const;
