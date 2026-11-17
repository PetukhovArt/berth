import type { Translation } from '../types';
import type { mobile as en } from '../en/mobile';

export const mobile: Translation<typeof en> = {
  back: 'Назад',
  navLabel: 'Разделы',
  commands: 'Команды',
  section: {
    inbox: 'Входящие',
    sessions: 'Сессии',
    reviews: 'Ревью',
    settings: 'Настройки',
  },
  summary: {
    needYou: 'ждут вас: {count}',
    running: 'работают: {count}',
    reviews: 'ждут вашего ревью: {count}',
    quiet: 'Ничего не ждёт вас',
  },
  connected: 'Подключено к berth',
  inboxEmpty: 'Сейчас ничего не ждёт вас.',
  keysLabel: 'Клавиши терминала',
  key: {
    esc: 'Esc',
    tab: 'Tab',
    ctrl: 'Ctrl',
    up: 'Вверх',
    down: 'Вниз',
    left: 'Влево',
    right: 'Вправо',
    newline: 'Shift+Enter',
  },
};
