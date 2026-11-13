import { computed, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';
import { BerthCallError, call } from './berth';

type ChannelSummary = { key: string; head: number; unread: number };
export type ChannelMessage = { seq: number; author: string; authorName: string; authorKind: string; content: string; createdAt: number };

const SUMMARY_POLL_MS = 30_000;

const channelPanelId = (key: string) => `channel:${key}`;

export type ChannelPresent = { member: string; name: string; agents: string[] };

export const isAgentMessage = (message: ChannelMessage) => message.authorKind === 'agent';

async function listChannels(): Promise<ChannelSummary[]> {
  const reply = await call({ ListChannels: {} });
  if (!('Channels' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.Channels.channels;
}

function messages(reply: Awaited<ReturnType<typeof call>>): ChannelMessage[] {
  if (!('ChannelEvents' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelEvents.events.map((event) => ({
    seq: event.seq,
    author: event.author,
    authorName: event.author_name,
    authorKind: event.author_kind,
    content: event.content,
    createdAt: event.created_at,
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
  return computed<LoadedChannelMessage[]>(() => [...loaded.value].flatMap(([channel, list]) => list.map((message) => ({ ...message, channel }))));
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

export function openChannel(key: string, split = false): void {
  useDock().openPanel({ id: channelPanelId(key), component: 'channel', title: `# ${key}`, params: { key } }, split);
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
  }
  return { unread: computed(() => channels.value.filter((channel) => channel.unread > 0)) };
}
