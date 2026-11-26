export const titleBar = {
  workspace: {
    label: 'Workspace: {name}',
    all: 'Все workspace',
    menu: 'Сменить workspace',
  },
  connectors: {
    unavailable: { one: '{count} коннектор недоступен', few: '{count} коннектора недоступны', many: '{count} коннекторов недоступны', other: '{count} коннектора недоступны' },
    tooltip: 'Не отвечают: {names}',
    title: 'Коннекторы · не отвечают: {count}',
    since: 'с {time}',
    retry: 'Повторить',
    configure: 'Настроить',
    vpnHint: 'Хосты не отвечают: проверьте VPN.',
  },
} as const;
