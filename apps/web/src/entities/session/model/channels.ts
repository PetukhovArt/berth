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

export async function readChannel(channel: string, after: number): Promise<ChannelMessage[]> {
  return messages(await call({ ReadChannel: { channel, after, agent: null } }));
}

export async function readPresence(channel: string): Promise<ChannelPresent[]> {
  const reply = await call({ GetChannelPresence: { channel } });
  if (!('ChannelPresence' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.ChannelPresence.here;
}

export async function postToChannel(channel: string, content: string): Promise<ChannelMessage[]> {
  return messages(await call({ PostToChannel: { channel, content, agent: null } }));
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
