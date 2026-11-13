export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type JsonObject = { [key: string]: Json };
export type GroupOp = 'all' | 'any';

export const ERROR_KINDS = ['usage_limit', 'overloaded', 'server', 'network', 'auth', 'model', 'other'] as const;
export const STATUSES = ['fresh', 'running', 'finished', 'needs_feedback', 'terminated', 'disconnected'] as const;
export const ROUTE_KINDS = ['claude', 'codex', 'cursor', 'opencode', 'grok'] as const;
const ALL_KINDS: readonly string[] = [...ROUTE_KINDS, 'acp'];

export type Condition =
  | { type: 'provider_error'; kinds: string[]; forSecs: number }
  | { type: 'limit_used'; pct: number; windowMinutes: number | null }
  | { type: 'status'; values: string[] }
  | { type: 'kind'; values: string[] }
  | { type: 'model'; values: string[] };
export type ConditionType = Condition['type'];

export type Row =
  | { id: string; type: 'condition'; negate: boolean; condition: Condition }
  | { id: string; type: 'opaque'; negate: boolean; raw: Json }
  | { id: string; type: 'join'; op: GroupOp }
  | { id: string; type: 'open'; negate: boolean; op: GroupOp }
  | { id: string; type: 'close' };

export type ConditionList = { root: GroupOp | null; rows: Row[] };

let nextId = 0;
const rowId = () => `row-${++nextId}`;

export function isObject(value: Json | undefined): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function single(value: Json): [string, Json] | null {
  if (!isObject(value)) return null;
  const entries = Object.entries(value);
  return entries.length === 1 ? (entries[0] ?? null) : null;
}

function group(value: Json): { op: GroupOp; items: Json[] } | null {
  const entry = single(value);
  if (!entry) return null;
  const [key, body] = entry;
  return (key === 'all' || key === 'any') && Array.isArray(body) ? { op: key, items: body } : null;
}

function onlyKeys(body: Json, keys: string[]): body is JsonObject {
  return isObject(body) && Object.keys(body).every((key) => keys.includes(key));
}

function stringsFrom(body: Json, allowed: readonly string[] | null): string[] | null {
  if (!Array.isArray(body) || !body.every((item) => typeof item === 'string')) return null;
  const values = body as string[];
  return allowed && !values.every((value) => allowed.includes(value)) ? null : values;
}

const isCount = (value: Json | undefined) => typeof value === 'number' && Number.isInteger(value) && value >= 0;

function parseCondition(value: Json): Condition | null {
  const entry = single(value);
  if (!entry) return null;
  const [key, body] = entry;
  if (key === 'provider_error') {
    if (!onlyKeys(body, ['kinds', 'for_secs'])) return null;
    const kinds = body.kinds === undefined ? [] : stringsFrom(body.kinds, ERROR_KINDS);
    const forSecs = body.for_secs ?? 0;
    if (!kinds || !isCount(forSecs)) return null;
    return { type: 'provider_error', kinds, forSecs: forSecs as number };
  }
  if (key === 'limit_used') {
    if (!onlyKeys(body, ['pct', 'window_minutes']) || typeof body.pct !== 'number') return null;
    const windowMinutes = body.window_minutes ?? null;
    if (windowMinutes !== null && !isCount(windowMinutes)) return null;
    return { type: 'limit_used', pct: body.pct, windowMinutes: windowMinutes as number | null };
  }
  if (key === 'status' || key === 'kind' || key === 'model') {
    const allowed = key === 'status' ? STATUSES : key === 'kind' ? ALL_KINDS : null;
    const values = stringsFrom(body, allowed);
    return values ? { type: key, values } : null;
  }
  return null;
}

function conditionJson(condition: Condition): Json {
  switch (condition.type) {
    case 'provider_error': {
      const body: JsonObject = { kinds: [...condition.kinds] };
      if (condition.forSecs > 0) body.for_secs = condition.forSecs;
      return { provider_error: body };
    }
    case 'limit_used': {
      const body: JsonObject = { pct: condition.pct };
      if (condition.windowMinutes !== null) body.window_minutes = condition.windowMinutes;
      return { limit_used: body };
    }
    default:
      return { [condition.type]: [...condition.values] };
  }
}

export function newCondition(type: ConditionType): Condition {
  switch (type) {
    case 'provider_error':
      return { type, kinds: ['usage_limit'], forSecs: 0 };
    case 'limit_used':
      return { type, pct: 95, windowMinutes: null };
    case 'status':
      return { type, values: ['finished'] };
    case 'kind':
      return { type, values: ['claude'] };
    case 'model':
      return { type, values: [] };
  }
}

function leafRows(value: Json, negate: boolean): Row[] {
  const condition = parseCondition(value);
  if (condition) return [{ id: rowId(), type: 'condition', negate, condition }];
  return [{ id: rowId(), type: 'opaque', negate, raw: value }];
}

function joined(items: Json[], op: GroupOp): Row[] {
  return items.flatMap((item, index) => (index ? [{ id: rowId(), type: 'join' as const, op }, ...nodeRows(item)] : nodeRows(item)));
}

function nodeRows(value: Json): Row[] {
  const asGroup = group(value);
  if (asGroup) return [{ id: rowId(), type: 'open', negate: false, op: asGroup.op }, ...joined(asGroup.items, asGroup.op), { id: rowId(), type: 'close' }];
  const entry = single(value);
  if (entry && entry[0] === 'not') {
    const inner = entry[1];
    const innerGroup = group(inner);
    if (innerGroup) return [{ id: rowId(), type: 'open', negate: true, op: innerGroup.op }, ...joined(innerGroup.items, innerGroup.op), { id: rowId(), type: 'close' }];
    if (parseCondition(inner)) return leafRows(inner, true);
  }
  return leafRows(value, false);
}

export function toConditionList(when: Json): ConditionList {
  const asGroup = group(when);
  if (asGroup) return { root: asGroup.op, rows: joined(asGroup.items, asGroup.op) };
  return { root: null, rows: nodeRows(when) };
}

function combine(items: Json[], ops: GroupOp[], declared: GroupOp | null, keepGroup: boolean): Json {
  const uniform = ops.every((op) => op === ops[0]);
  if (uniform) {
    if (!keepGroup && declared === null && items.length === 1 && items[0] !== undefined) return items[0];
    return { [ops[0] ?? declared ?? 'any']: items };
  }
  const runs: Json[][] = [[]];
  items.forEach((item, index) => {
    if (index > 0 && ops[index - 1] === 'any') runs.push([]);
    runs[runs.length - 1]?.push(item);
  });
  return { any: runs.map((run) => (run.length === 1 && run[0] !== undefined ? run[0] : { all: run })) };
}

const negated = (value: Json, negate: boolean): Json => (negate ? { not: value } : value);

export function fromConditionList(list: ConditionList): Json {
  let at = 0;
  const level = (): { items: Json[]; ops: GroupOp[] } => {
    const items: Json[] = [];
    const ops: GroupOp[] = [];
    while (at < list.rows.length) {
      const row = list.rows[at++];
      if (!row || row.type === 'close') break;
      if (row.type === 'join') ops.push(row.op);
      else if (row.type === 'condition') items.push(negated(conditionJson(row.condition), row.negate));
      else if (row.type === 'opaque') items.push(negated(row.raw, row.negate));
      else {
        const inner = level();
        items.push(negated(combine(inner.items, inner.ops, row.op, true), row.negate));
      }
    }
    return { items, ops };
  };
  const top = level();
  return combine(top.items, top.ops, list.root, false);
}

export function depthOf(rows: Row[], index: number): number {
  let depth = 0;
  for (let i = 0; i < index; i++) {
    const row = rows[i];
    if (row?.type === 'open') depth++;
    if (row?.type === 'close') depth--;
  }
  return rows[index]?.type === 'close' ? depth - 1 : depth;
}

const isValue = (row: Row | undefined) => row?.type === 'condition' || row?.type === 'opaque' || row?.type === 'close';

export function closeOf(rows: Row[], openIndex: number): number {
  let depth = 0;
  for (let i = openIndex; i < rows.length; i++) {
    const row = rows[i];
    if (row?.type === 'open') depth++;
    if (row?.type === 'close' && --depth === 0) return i;
  }
  return rows.length - 1;
}

export function insertRows(rows: Row[], at: number, inserted: Row[]): Row[] {
  const before = rows[at - 1];
  const join = (op: GroupOp): Row => ({ id: rowId(), type: 'join', op });
  if (isValue(before)) return [...rows.slice(0, at), join('all'), ...inserted, ...rows.slice(at)];
  const after = rows[at];
  if (after && after.type !== 'close' && after.type !== 'join') return [...rows.slice(0, at), ...inserted, join('all'), ...rows.slice(at)];
  return [...rows.slice(0, at), ...inserted, ...rows.slice(at)];
}

export function conditionRow(type: ConditionType): Row {
  return { id: rowId(), type: 'condition', negate: false, condition: newCondition(type) };
}

export function groupRows(): Row[] {
  return [
    { id: rowId(), type: 'open', negate: false, op: 'any' },
    { id: rowId(), type: 'close' },
  ];
}

export function removeRows(rows: Row[], from: number, to: number): Row[] {
  let start = from;
  let end = to;
  if (rows[start - 1]?.type === 'join') start--;
  else if (rows[end + 1]?.type === 'join') end++;
  return [...rows.slice(0, start), ...rows.slice(end + 1)];
}
