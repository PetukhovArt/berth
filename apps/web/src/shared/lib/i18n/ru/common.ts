import type { Translation } from '../types';
import type { common as en } from '../en/common';

export const common: Translation<typeof en> = {
  cancel: 'Отмена',
  save: 'Сохранить',
  close: 'Закрыть',
  delete: 'Удалить',
  remove: 'Убрать',
  create: 'Создать',
  confirm: 'Подтвердить',
  rename: 'Переименовать',
  retry: 'Повторить',
  refresh: 'Обновить',
  copy: 'Копировать',
  back: 'Назад',
  open: 'Открыть',
  openInBrowser: 'Открыть в браузере',
  search: 'Поиск',
  searchPlaceholder: 'Поиск по сессиям, MR, задачам и перепискам',
  loading: 'Загрузка…',
  error: 'Ошибка',
  noResults: 'Ничего не найдено',
  time: {
    justNow: 'только что',
  },
  kit: {
    emptyMessage: 'Нет доступных вариантов',
    emptySearchMessage: 'Ничего не найдено',
    searchMessage: 'Найдено результатов: {0}',
    selectionMessage: 'Выбрано: {0}',
    emptySelectionMessage: 'Ничего не выбрано',
    selectAll: 'Выбраны все элементы',
    unselectAll: 'Выбор снят со всех элементов',
  },
  language: {
    label: 'Язык',
    en: 'English',
    ru: 'Русский',
  },
};
