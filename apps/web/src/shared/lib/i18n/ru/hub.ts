import type { Translation } from '../types';
import type { hub as en } from '../en/hub';

export const hub: Translation<typeof en> = {
  category: 'Команда',
  keywords: 'команда хаб участники участник роли роль права разрешения журнал аудит админ одобрить отозвать пригласить гость каналы доступ',
  title: 'Хаб команды',
  refused: 'Роль {role} этого не разрешает: {permission}',
  permission: {
    channels: {
      read: 'Читать каналы',
      post: 'Писать в каналы',
      create: 'Создавать каналы',
      invite: 'Приглашать в каналы',
      delete: 'Удалять каналы',
    },
    agents: {
      join: 'Подключать агентов',
      instruct: 'Давать поручения агентам',
    },
    context: {
      write: 'Задавать заметки контекста',
    },
    admin: {
      manage: 'Управлять хабом',
    },
  },
  permissionHint: {
    channels: {
      read: 'Видеть каналы, в которых вы состоите, и их сообщения.',
      post: 'Писать сообщения в канал.',
      create: 'Открывать новый канал.',
      invite: 'Добавлять других людей в канал.',
      delete: 'Удалять любой канал вместе с сообщениями. Без этого права удаляются только свои каналы.',
    },
    agents: {
      join: 'Подключать своих агентов к каналам, чтобы они работали от вашего имени.',
      instruct: 'Разрешить вашим сообщениям давать поручения чужим агентам.',
    },
    context: {
      write: 'Задавать общую заметку, которую читают агенты канала.',
    },
    admin: {
      manage: 'Одобрять людей, менять роли и читать журнал.',
    },
  },
  group: {
    channels: 'Каналы',
    agents: 'Агенты',
    context: 'Контекст',
    admin: 'Управление',
  },
  state: {
    loading: 'Загружаю хаб команды…',
    off: 'Хаб команды не подключён. Задайте эти ключи в config.json:',
    legacy: 'Этот хаб старше ролей. Обновите хаб, чтобы управлять участниками и ролями здесь.',
    unreachable: 'Не удаётся связаться с хабом команды: {error}',
  },
  me: {
    who: 'Вы: {name}',
    role: 'Роль: {role}',
    grants: 'Ваша роль разрешает:',
    grantsNone: 'Ваша роль ничего не разрешает.',
    notAdmin: 'Участников и роли меняет только админ.',
  },
  members: {
    title: 'Участники',
    empty: 'Пока никто не присоединился.',
    waiting: 'Ждут одобрения: {count}',
    role: 'Роль участника {name}',
    approveAs: 'Одобрить как…',
    self: 'Своё устройство меняет другой админ или владелец admin token.',
    revoke: 'Отозвать',
    revokeQuestion: 'Отозвать доступ у {name}? Устройство потеряет доступ, пока не подключится заново.',
    revokeConfirm: 'Отозвать',
    status: {
      pending: 'Ждёт',
      approved: 'Одобрен',
      revoked: 'Отозван',
    },
  },
  roles: {
    title: 'Роли',
    empty: 'Ролей пока нет.',
    builtin: 'Встроенная',
    noPermissions: 'Нет разрешений',
    heldBy: { one: 'У {count} участника', few: 'У {count} участников', many: 'У {count} участников', other: 'У {count} участников' },
    deleteHeld: { one: 'Нельзя удалить: роль у {count} участника', few: 'Нельзя удалить: роль у {count} участников', many: 'Нельзя удалить: роль у {count} участников', other: 'Нельзя удалить: роль у {count} участников' },
    new: 'Новая роль',
    edit: 'Изменить',
    delete: 'Удалить',
    deleteLabel: 'Удалить роль {name}',
    editLabel: 'Изменить роль {name}',
    name: 'Название роли',
    namePlaceholder: 'Например, Ревьюер',
    permissions: 'Разрешения',
    save: 'Сохранить роль',
    editing: 'Изменение: {name}',
    creating: 'Новая роль',
  },
  audit: {
    title: 'Журнал',
    empty: 'Изменений пока нет.',
    roleCreated: '{actor} создал роль {role}',
    roleSaved: '{actor} изменил роль {role}',
    roleDeleted: '{actor} удалил роль {role}',
    memberApproved: '{actor} одобрил {member} с ролью {role}',
    memberRole: '{actor} сменил роль {member} на {role}',
    memberRevoked: '{actor} отозвал доступ у {member}',
    channelDeleted: '{actor} удалил канал {channel}',
    migrated: 'Хаб перешёл на роли. Каждое одобренное устройство стало участником с ролью Member.',
    unknown: '{actor}: {action} {target}',
  },
};
