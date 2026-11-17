import { acpMessages } from './acpChat';
import { acpTranscript, useBerth, type SessionNode } from './berth';
import { readTranscript, type ExternalSession, type TranscriptMessage } from './external';

export type SessionTextCandidate = { key: string; stamp: number; session?: SessionNode; external?: ExternalSession; linked?: ExternalSession };
type SessionTextHit ={ key: string; text: string };

const WINDOW = 60;
const MAX_SESSIONS = 30;
const CONCURRENCY = 4;
const MAX_CACHED = 200;
const SNIPPET_BEFORE = 24;
const SNIPPET_LENGTH = 110;

type Source = { identity: string; load: () => Promise<TranscriptMessage[]> };

const cache = new Map<string, { stamp: number; texts: string[] }>();

function sourceOf(candidate: SessionTextCandidate): Source | null {
  const own = candidate.external ?? candidate.linked;
  if (own) return { identity: `${own.harness}:${own.id}`, load: () => readTranscript(own, WINDOW) };
  const ref = candidate.session?.ref;
  if (!ref || !('Agent' in ref)) return null;
  const agentId = ref.Agent;
  const agent = useBerth()?.entities.value.agents.get(agentId);
  if (agent?.kind === 'acp') return { identity: `acp:${agentId}`, load: () => acpTranscript(agentId, 0).then((items) => acpMessages(items.slice(-WINDOW))) };
  if (agent?.kind === 'claude' && agent.session_id) {
    const id = agent.session_id;
    return { identity: `claude:${id}`, load: () => readTranscript({ harness: 'claude', id }, WINDOW) };
  }
  return null;
}

async function textsOf(source: Source, stamp: number): Promise<string[]> {
  const cached = cache.get(source.identity);
  if (cached && cached.stamp === stamp) return cached.texts;
  let texts: string[] = [];
  try {
    texts = (await source.load()).filter((message) => message.tool === null && !message.shell && message.text.trim()).map((message) => message.text);
  } catch {
    texts = [];
  }
  if (cache.size >= MAX_CACHED) cache.clear();
  cache.set(source.identity, { stamp, texts });
  return texts;
}

function snippet(text: string, word: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const at = Math.max(0, flat.toLowerCase().indexOf(word));
  const start = Math.max(0, at - SNIPPET_BEFORE);
  const end = Math.min(flat.length, start + SNIPPET_LENGTH);
  return `${start > 0 ? '…' : ''}${flat.slice(start, end)}${end < flat.length ? '…' : ''}`;
}

function lastMatch(texts: readonly string[], words: readonly string[]): string | null {
  for (let at = texts.length - 1; at >= 0; at--) {
    const text = texts[at]!;
    const lower = text.toLowerCase();
    if (words.every((word) => lower.includes(word))) return snippet(text, words[0]!);
  }
  return null;
}

export async function searchSessionText(candidates: readonly SessionTextCandidate[], words: readonly string[]): Promise<SessionTextHit[]> {
  if (!words.length) return [];
  const seen = new Set<string>();
  const picked: { candidate: SessionTextCandidate; source: Source }[] = [];
  for (const candidate of [...candidates].sort((a, b) => b.stamp - a.stamp)) {
    if (picked.length >= MAX_SESSIONS) break;
    const source = sourceOf(candidate);
    if (!source || seen.has(source.identity)) continue;
    seen.add(source.identity);
    picked.push({ candidate, source });
  }
  const found: (SessionTextHit | null)[] = picked.map(() => null);
  let next = 0;
  const work = async () => {
    while (next < picked.length) {
      const at = next++;
      const { candidate, source } = picked[at]!;
      const text = lastMatch(await textsOf(source, candidate.stamp), words);
      if (text) found[at] = { key: candidate.key, text };
    }
  };
  await Promise.all(Array.from({ length: Math.min(CONCURRENCY, picked.length) }, work));
  return found.filter((hit): hit is SessionTextHit => hit !== null);
}
