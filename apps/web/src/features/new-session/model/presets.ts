import { computed, shallowRef } from 'vue';

const KEY = 'berth.agent.presets';

export interface AgentPreset {
  name: string;
  kind: string;
  model: string;
  effort: string;
  before: string;
  after: string;
}

function read(): AgentPreset[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? '[]');
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is Record<string, unknown> => !!item && typeof item === 'object' && typeof (item as { name?: unknown }).name === 'string')
      .map((item) => ({
        name: String(item.name),
        kind: String(item.kind ?? 'claude'),
        model: String(item.model ?? ''),
        effort: String(item.effort ?? ''),
        before: String(item.before ?? ''),
        after: String(item.after ?? ''),
      }));
  } catch {
    return [];
  }
}

const stored = shallowRef<AgentPreset[]>(read());

function write(next: AgentPreset[]) {
  stored.value = next;
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    return;
  }
}

export const presets = computed(() => stored.value);

export function savePreset(preset: AgentPreset) {
  write([...stored.value.filter((item) => item.name !== preset.name), preset]);
}

export function deletePreset(name: string) {
  write(stored.value.filter((item) => item.name !== name));
}

export function withPromptFrame(frame: { before: string; after: string }, prompt: string): string | null {
  const text = [frame.before, prompt, frame.after]
    .map((part) => part.trim())
    .filter(Boolean)
    .join('\n\n');
  return text || null;
}
