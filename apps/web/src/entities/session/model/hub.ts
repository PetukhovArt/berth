import { computed, getCurrentScope, onScopeDispose, shallowRef, type ComputedRef } from 'vue';
import CATALOG from '../../../../../../crates/berth-hub/permissions.json';
import { t } from '../../../shared/lib/i18n/i18n';
import { BerthCallError, call, describeError, onHubChanged } from './berth';

type Permission = keyof typeof CATALOG;

type RoleRef = { id: string; name: string };
type HubMe = { id: string; name: string; role: RoleRef; permissions: string[] };
export type HubRole = { id: string; name: string; builtin: boolean; permissions: string[]; members: number };
type HubMemberStatus = 'pending' | 'approved' | 'revoked';
export type HubMember = { id: string; name: string; status: HubMemberStatus; role: string | null };
export type HubAuditEntry = { seq: number; at: number; actor: string; actor_name: string; action: string; target: string; detail: unknown };
export type HubAdmin = { catalog: string[]; roles: HubRole[]; members: HubMember[]; audit: HubAuditEntry[] };
export type HubView = { state: 'off' } | { state: 'legacy' } | { state: 'joined'; me: HubMe; admin: HubAdmin | null };

export type HubChange =
  | { SaveRole: { id: string | null; name: string; permissions: string[] } }
  | { DeleteRole: { id: string } }
  | { SetRole: { member: string; role: string } }
  | { Revoke: { member: string } };

type Verdict = { ok: true } | { ok: false; reason: string };
type HubState = 'loading' | 'off' | 'legacy' | 'unreachable' | 'member' | 'admin';

const view = shallowRef<HubView | null>(null);
const failure = shallowRef<string | null>(null);
const refreshing = shallowRef(false);
let generation = 0;

function parseView(json: string): HubView {
  const value: unknown = JSON.parse(json);
  const record = typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : {};
  const known = record.state === 'off' || record.state === 'legacy' || (record.state === 'joined' && typeof record.me === 'object' && record.me !== null);
  if (!known) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: json }));
  return value as HubView;
}

function unwrap(reply: Awaited<ReturnType<typeof call>>): HubView {
  if (!('Hub' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return parseView(reply.Hub.json);
}

export async function refreshHub(): Promise<void> {
  const mine = ++generation;
  refreshing.value = true;
  try {
    const next = unwrap(await call({ GetHub: {} }));
    if (mine !== generation) return;
    view.value = next;
    failure.value = null;
  } catch (error) {
    if (mine === generation) failure.value = describeError(error);
  } finally {
    if (mine === generation) refreshing.value = false;
  }
}

export async function changeHub(change: HubChange): Promise<void> {
  const next = unwrap(await call({ ChangeHub: { change: JSON.stringify(change) } }));
  generation++;
  refreshing.value = false;
  view.value = next;
  failure.value = null;
}

let users = 0;
let stopListening: (() => void) | null = null;

function release() {
  users--;
  if (users > 0) return;
  stopListening?.();
  stopListening = null;
}

function retain() {
  if (users++ === 0) {
    stopListening = onHubChanged(() => void refreshHub());
    void refreshHub();
  }
  if (getCurrentScope()) onScopeDispose(release);
}

const isKnown = (key: string): key is Permission => Object.hasOwn(CATALOG, key);

export function permissionLabel(key: string): string {
  return isKnown(key) ? t(`hub.permission.${key}`) : key;
}

export function permissionHint(key: string): string {
  return isKnown(key) ? t(`hub.permissionHint.${key}`) : '';
}

const joined = (value: HubView | null) => (value?.state === 'joined' ? value : null);

function verdict(value: HubView | null, permission: Permission): Verdict {
  const me = joined(value)?.me;
  if (!me || me.permissions.includes(permission)) return { ok: true };
  return { ok: false, reason: t('hub.refused', { role: me.role.name, permission: permissionLabel(permission) }) };
}

export function useMay(permission: Permission): ComputedRef<Verdict> {
  retain();
  return computed(() => verdict(view.value, permission));
}

export function useHubAdmin() {
  retain();
  return {
    me: computed(() => joined(view.value)?.me ?? null),
    admin: computed(() => joined(view.value)?.admin ?? null),
    state: computed<HubState>(() => {
      if (failure.value !== null) return 'unreachable';
      const value = view.value;
      if (value === null) return 'loading';
      if (value.state === 'joined') return value.admin ? 'admin' : 'member';
      return value.state;
    }),
    failure,
    refreshing,
  };
}
