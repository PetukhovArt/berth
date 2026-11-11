export const hot = {
  title: 'Горячие сессии',
  switcher: {
    dialog: 'Переключение сессии',
    empty: 'Нет живых сессий',
  },
  status: {
    waiting: 'Ждёт вас',
    working: 'Работает',
    idle: 'Простаивает',
    error: 'Ошибка',
  },
  overview: {
    title: 'Обзор горячих сессий',
    hint: 'Стрелки для перемещения, Enter чтобы открыть, Esc чтобы закрыть',
    close: 'Закрыть обзор',
    empty: 'Нет живых сессий',
    noPreview: 'Пока без активности',
  },
  toast: {
    waiting: 'Ждут: {count}',
    nobody: 'Никто не ждёт',
  },
  mode: {
    titlebar: 'Включено',
    off: 'Выключено',
  },
  setting: {
    label: 'Горячие сессии',
    description: 'Живые сессии под рукой: обзор по {key:mod-shift-o}, {key:alt-1}..9 открывает N-ю, {key:alt-j} переходит к следующей, которая ждёт вас, {key:ctrl-tab} переключает между недавними.',
  },
} as const;
