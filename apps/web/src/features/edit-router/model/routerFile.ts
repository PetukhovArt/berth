import { BerthCallError, call } from '../../../entities/session';
import { fromConditionList, isObject, toConditionList, type ConditionList, type Json, type JsonObject } from './conditionRows';

type ChainEntry = { kind: string; model: string };
export type RuleDraft = { raw: JsonObject; id: string; name: string; enabled: boolean; cooldownSecs: number | null; conditions: ConditionList; actions: Json[] };
export type RouterDraft = { raw: JsonObject; mode: string; fallback: ChainEntry[]; rules: RuleDraft[] };
export type Loaded = { draft: RouterDraft; path: string };

const ACTIONS_KEY = `then`;
const text = (value: Json | undefined, fallback = '') => (typeof value === 'string' ? value : fallback);

function toEntry(entry: Json): ChainEntry {
  const [kind = '', model = ''] = text(entry).split(/:(.*)/s);
  return { kind: kind.trim(), model: model.trim() };
}

function toRule(value: Json): RuleDraft {
  const raw = isObject(value) ? value : {};
  return {
    raw,
    id: text(raw.id),
    name: text(raw.name),
    enabled: raw.enabled !== false,
    cooldownSecs: typeof raw.auto_cooldown_secs === 'number' ? raw.auto_cooldown_secs : null,
    conditions: toConditionList(raw.when ?? { any: [] }),
    actions: Array.isArray(raw[ACTIONS_KEY]) ? raw[ACTIONS_KEY] : [],
  };
}

function toDraft(json: string): RouterDraft {
  const parsed = JSON.parse(json) as Json;
  const raw = isObject(parsed) ? parsed : {};
  return {
    raw,
    mode: text(raw.mode, 'suggest'),
    fallback: (Array.isArray(raw.fallback) ? raw.fallback : []).map(toEntry),
    rules: (Array.isArray(raw.rule) ? raw.rule : []).map(toRule),
  };
}

function ruleJson(rule: RuleDraft): JsonObject {
  const out: JsonObject = { ...rule.raw, id: rule.id, name: rule.name, enabled: rule.enabled, when: fromConditionList(rule.conditions) };
  out[ACTIONS_KEY] = rule.actions;
  if (rule.cooldownSecs === null) delete out.auto_cooldown_secs;
  else out.auto_cooldown_secs = rule.cooldownSecs;
  return out;
}

export function draftJson(draft: RouterDraft): string {
  const fallback = draft.fallback.filter((entry) => entry.kind).map((entry) => (entry.model ? `${entry.kind}:${entry.model}` : entry.kind));
  return JSON.stringify({ ...draft.raw, mode: draft.mode, fallback, rule: draft.rules.map(ruleJson) });
}

export function newRule(taken: string[]): RuleDraft {
  let n = taken.length + 1;
  while (taken.includes(`rule-${n}`)) n++;
  return { ...toRule({ id: `rule-${n}`, name: '', enabled: true, when: { any: [] } }), actions: [{ next_in_chain: {} }] };
}

async function routerCall(request: Record<string, Record<string, unknown>>): Promise<Loaded> {
  const reply = await call(request);
  if (!('RouterConfig' in reply)) throw new BerthCallError(Object.keys(reply).join(''));
  return { draft: toDraft(reply.RouterConfig.json), path: reply.RouterConfig.path };
}

export const loadRouter = () => routerCall({ GetRouterConfig: {} });
export const saveRouter = (draft: RouterDraft) => routerCall({ SaveRouterConfig: { json: draftJson(draft) } });
