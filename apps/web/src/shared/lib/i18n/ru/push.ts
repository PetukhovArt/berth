import type { Translation } from '../types';
import type { push as en } from '../en/push';

export const push: Translation<typeof en> = {
  title: 'Уведомления',
  unsupported: 'Этот браузер не принимает push-уведомления. Откройте Berth по HTTPS (Tailscale HTTPS) и добавьте на экран «Домой».',
  denied: 'Уведомления для этого сайта заблокированы в настройках браузера.',
  off: 'Получайте уведомление здесь, когда Агент ждёт вас.',
  on: 'Это устройство получит уведомление, когда Агент ждёт вас.',
  enable: 'Включить уведомления',
  disable: 'Выключить уведомления',
  allow: 'Разрешить',
  deny: 'Отклонить',
  failed: 'Не удалось связаться с Berth. Откройте приложение, чтобы ответить.',
};
