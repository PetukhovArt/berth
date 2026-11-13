import type { Translation } from '../types';
import type { remote as en } from '../en/remote';

export const remote: Translation<typeof en> = {
  category: 'Удалённый доступ',
  def: {
    enabled: {
      label: 'Удалённый доступ',
      description: 'Открыть Berth для телефона или браузера по сети. Любой, у кого есть ссылка подключения, управляет вашими терминалами.',
    },
    bind: {
      label: 'Слушать',
      description: 'Tailscale — только адрес 100.x этой машины, Tailscale (HTTPS) — сервер остаётся на localhost и публикуется через tailscale serve по адресу https://<машина>.ts.net, локальная сеть — все сетевые интерфейсы, localhost — только эта машина.',
      localhost: 'Только эта машина (localhost)',
      lan: 'Локальная сеть (LAN)',
      tailscale: 'Tailscale',
      tailscaleHttps: 'Tailscale (HTTPS)',
    },
    port: {
      label: 'Порт',
      description: 'TCP-порт сервера удалённого доступа.',
    },
  },
  panel: {
    warningTitle: 'Удалённый доступ даёт полное управление терминалами',
    warning: 'Подключённое устройство может вводить текст в любую сессию, запускать команды и читать файлы рабочих деревьев. Трафик идёт по обычному HTTP: лучше используйте Tailscale, он его шифрует, и никогда не открывайте порт в интернет.',
    off: 'Удалённый доступ выключен. Включите его ниже, чтобы получить ссылку подключения.',
    starting: 'Сервер запускается…',
    running: 'Сервер работает',
    addresses: 'Адреса',
    pairing: 'Ссылка подключения',
    pairingHint: 'Отсканируйте QR-код камерой телефона или откройте ссылку в браузере. Устройство остаётся подключённым, пока токен не перевыпущен.',
    copy: 'Скопировать ссылку',
    copied: 'Скопировано',
    regenerate: 'Перевыпустить токен',
    regenerateHint: 'Создаёт новую ссылку подключения и отключает все подключённые устройства.',
    clients: 'Подключённые устройства',
    noClients: 'Нет подключённых устройств',
    connectedAt: 'с {time}',
    qrLabel: 'QR-код ссылки подключения',
    installTailscale: 'Установить Tailscale',
    useLan: 'Использовать локальную сеть',
    desktopOnly: 'Удалённым доступом управляют в приложении Berth на компьютере. На хосте с berth-server (Docker) ссылка подключения и QR-код есть в логе сервера: docker compose -f docker/compose.yaml logs berth.',
    problem: {
      tailscale_missing: 'У этого компьютера нет адреса Tailscale (100.x). Установите Tailscale и войдите в один и тот же аккаунт на этом компьютере и на телефоне или переключитесь на локальную сеть (LAN), если телефон в той же Wi-Fi.',
      serve_failed: 'Не удалось опубликовать сервер через Tailscale Serve: {message}',
      bind_failed: 'Не удалось слушать {message}',
      token_store: 'Не удалось прочитать токен подключения из системного хранилища ключей: {message}',
    },
  },
};
