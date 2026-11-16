import { computed, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';
import { BerthCallError, call, onChannelAppended, useBerth } from './berth';

export const agentName = (agent: string) => useBerth()?.entities.value.agents.get(agent)?.name ?? agent;

export { onChannelAppended };

type ChannelSummary = { key: string; head: number; unread: number };
export type ChannelMessage = { seq: number; kind: number; author: string; authorName: string; authorKind: string; tags: string[][]; content: string; createdAt: number; authorAgent: string | null; promotable: boolean };
export type ChannelTouching = { member: string; name: string; agent: string; modules: string[] };
export type ChannelGrant = { channel: string; agent: string; member: string; member_name: string };

const KIND_MESSAGE = 1;
const KIND_REQUEST = 2;
const KIND_ANSWER = 3;
const KIND_SETTLED = 4;
const NOTE_KIND = 5;

export type ChannelRequest = { kind: 'permission' | 'question'; title: string; detail: string | null; options: string[] };
export type ChannelSettlement = { answer: string; by: string | null };

const tagValues = (message: ChannelMessage, name: string) => message.tags.filter((tag) => tag[0] === name && tag.length === 2).map((tag) => tag[1] ?? '');

export const isChannelReply = (message: ChannelMessage) => message.kind === KIND_ANSWER || message.kind === KIND_SETTLED;

export function channelRequestOf(message: ChannelMessage): ChannelRequest | null {
  if (message.kind !== KIND_REQUEST) return null;
  const kind = tagValues(message, 'request')[0];
  if (kind !== 'permission' && kind !== 'question') return null;
  return { kind, title: tagValues(message, 'title')[0] ?? '', detail: tagValues(message, 'detail')[0] ?? null, options: tagValues(message, 'option') };
}

export function settlementOf(messages: readonly ChannelMessage[], request: ChannelMessage): ChannelSettlement | null {
  const seq = String(request.seq);
  const settled = messages.find((message) => message.kind === KIND_SETTLED && message.author === request.author && tagValues(message, 'e').includes(seq));
  if (!settled) return null;
  return { answer: tagValues(settled, 'answer')[0] ?? '', by: tagValues(settled, 'by')[0] ?? null };
}

export const isOwnAgentRequest = (message: ChannelMessage) => message.authorAgent !== null;

const SUMMARY_POLL_MS = 30_000;

const channelPanelId = (key: string) => `channel:${key}`;

export type ChannelPresent = { member: string; name: string; agents: string[] };

export const isAgentMessage = (message: ChannelMessage) => message.authorKind === 'agent';
export const isNoteMessage = (message: ChannelMessage) => message.kind === NOTE_KIND;

async function listChannels(): Promise<ChannelSummary[]> {
  const reply = await call({ ListChannels: {} });
  if (!('Channels' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.Channels.channels;
}

function messages(reply: Awaited<ReturnType<typeof call>>): ChannelMessage[] {
  if (!('ChannelEvents' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelEvents.events.map((event) => ({
    seq: event.seq,
    kind: event.kind,
    author: event.author,
    authorName: event.author_name,
    authorKind: event.author_kind,
    tags: event.tags,
    content: event.content,
    createdAt: event.created_at,
    authorAgent: event.author_agent,
    promotable: event.kind === KIND_MESSAGE && !event.from_me && event.mentioned_agents.length > 0,
  }));
}

const SEARCH_CACHE_PER_CHANNEL = 200;
const loaded = shallowRef<ReadonlyMap<string, readonly ChannelMessage[]>>(new Map());

function remember(channel: string, fresh: ChannelMessage[]): ChannelMessage[] {
  if (!fresh.length) return fresh;
  const known = loaded.value.get(channel) ?? [];
  const seen = new Set(known.map((message) => message.seq));
  const merged = [...known, ...fresh.filter((message) => !seen.has(message.seq))].sort((a, b) => a.seq - b.seq).slice(-SEARCH_CACHE_PER_CHANNEL);
  loaded.value = new Map(loaded.value).set(channel, merged);
  return fresh;
}

export type LoadedChannelMessage = ChannelMessage & { channel: string };

export function useLoadedChannelMessages() {
  return computed<LoadedChannelMessage[]>(() => [...loaded.value].flatMap(([channel, list]) => list.filter((message) => !isChannelReply(message)).map((message) => ({ ...message, channel }))));
}

export async function readChannel(channel: string, after: number): Promise<ChannelMessage[]> {
  return remember(channel, messages(await call({ ReadChannel: { channel, after, agent: null } })));
}

export async function readPresence(channel: string): Promise<ChannelPresent[]> {
  const reply = await call({ GetChannelPresence: { channel } });
  if (!('ChannelPresence' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelPresence.here;
}

export async function postToChannel(channel: string, content: string): Promise<ChannelMessage[]> {
  return remember(channel, messages(await call({ PostToChannel: { channel, content, agent: null } })));
}

export async function postNote(channel: string, content: string): Promise<ChannelMessage[]> {
  return remember(channel, messages(await call({ PostChannelNote: { channel, content, agent: null } })));
}

export async function readTouched(channel: string): Promise<ChannelTouching[]> {
  const reply = await call({ GetChannelTouched: { channel } });
  if (!('ChannelTouched' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelTouched.touched;
}

export async function channelKeys(): Promise<string[]> {
  return (await listChannels()).map((channel) => channel.key);
}

export async function answerInChannel(channel: string, request: number, answer: string): Promise<ChannelMessage[]> {
  return remember(channel, messages(await call({ AnswerInChannel: { channel, request, answer, agent: null } })));
}

export async function promoteChannelMessage(channel: string, seq: number): Promise<void> {
  const reply = await call({ PromoteChannelMessage: { channel, seq, agent: null } });
  if (!('Ack' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
}

function grants(reply: Awaited<ReturnType<typeof call>>): ChannelGrant[] {
  if (!('ChannelGrants' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelGrants.grants;
}

export async function readChannelGrants(channel: string): Promise<ChannelGrant[]> {
  return grants(await call({ ListChannelGrants: { channel } }));
}

export async function revokeChannelGrant(grant: ChannelGrant): Promise<ChannelGrant[]> {
  return grants(await call({ SetChannelGrant: { channel: grant.channel, agent: grant.agent, member: grant.member, granted: false } }));
}

export function openChannel(key: string, split = false): void {
  useDock().openPanel({ id: channelPanelId(key), component: 'channel', title: `# ${key}`, params: { key } }, split);
}

export async function openChannelFor(key: string): Promise<void> {
  await call({ OpenChannel: { key, agent: null } }).catch(() => undefined);
  openChannel(key);
}

const channels = shallowRef<ChannelSummary[]>([]);
let started = false;
let inflight = false;

export async function refreshChannels(): Promise<void> {
  if (inflight) return;
  inflight = true;
  try {
    channels.value = await listChannels();
  } catch {
    channels.value = [];
  } finally {
    inflight = false;
  }
}

export function useChannelUnread() {
  if (!started) {
    started = true;
    void refreshChannels();
    setInterval(() => void refreshChannels(), SUMMARY_POLL_MS);
    onChannelAppended(() => void refreshChannels());
  }
  return { unread: computed(() => channels.value.filter((channel) => channel.unread > 0)) };
}
