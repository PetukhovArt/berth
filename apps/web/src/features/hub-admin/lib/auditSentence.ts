import { t } from '../../../shared/lib/i18n/i18n';
import type { HubAdmin, HubAuditEntry } from '../../../entities/session';

type Detail = { before: unknown; after: unknown };

function parseDetail(value: unknown): Detail {
  if (typeof value !== 'object' || value === null) return { before: null, after: null };
  const record = value as Record<string, unknown>;
  return { before: record.before ?? null, after: record.after ?? null };
}

function field(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null ? (value as Record<string, unknown>)[key] : undefined;
}

function roleName(value: unknown, admin: HubAdmin): string | null {
  if (typeof value === 'string') return admin.roles.find((role) => role.id === value)?.name ?? value;
  const name = field(value, 'name');
  if (typeof name === 'string') return name;
  const role = field(value, 'role');
  return role === undefined || role === value ? null : roleName(role, admin);
}

export function auditSentence(entry: HubAuditEntry, admin: HubAdmin): string {
  const actor = entry.actor_name || entry.actor;
  const detail = parseDetail(entry.detail);
  const roleOf = (id: string) => admin.roles.find((role) => role.id === id)?.name ?? roleName(detail.after, admin) ?? roleName(detail.before, admin) ?? id;
  const memberOf = (id: string) => admin.members.find((member) => member.id === id)?.name ?? id;
  switch (entry.action) {
    case 'role.saved':
      return t(detail.before === null ? 'hub.audit.roleCreated' : 'hub.audit.roleSaved', { actor, role: roleOf(entry.target) });
    case 'role.deleted':
      return t('hub.audit.roleDeleted', { actor, role: roleName(detail.before, admin) ?? entry.target });
    case 'member.role': {
      const role = roleName(detail.after, admin) ?? '';
      return t(detail.before === null || roleName(detail.before, admin) === null ? 'hub.audit.memberApproved' : 'hub.audit.memberRole', { actor, member: memberOf(entry.target), role });
    }
    case 'member.revoked':
      return t('hub.audit.memberRevoked', { actor, member: memberOf(entry.target) });
    case 'channel.deleted':
      return t('hub.audit.channelDeleted', { actor, channel: entry.target });
    case 'hub.migrated':
      return t('hub.audit.migrated');
    default:
      return t('hub.audit.unknown', { actor, action: entry.action, target: entry.target });
  }
}
