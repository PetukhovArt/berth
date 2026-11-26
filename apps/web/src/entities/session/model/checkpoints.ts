import { t } from '../../../shared/lib/i18n/i18n';
import { computed, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue';
import { useSettings } from '../../../shared/config/settings/store';
import { acknowledged, BerthCallError, call, describeError, type AgentKind } from './berth';

export type Checkpoint = { number: number; created_at: number; before_restore: boolean; changed: boolean };

export async function listCheckpoints(agent: string): Promise<Checkpoint[]> {
  const reply = await call({ ListCheckpoints: { agent } });
  if (!('Checkpoints' in reply)) throw new BerthCallError(t('agents.errors.unexpectedReply', { reply: Object.keys(reply).join('') }));
  return reply.Checkpoints.checkpoints;
}

export async function restoreCheckpoint(agent: string, number: number): Promise<void> {
  await acknowledged({ RestoreCheckpoint: { agent, number } });
}

export function useCheckpoints(agent: MaybeRefOrGetter<string>, kind: MaybeRefOrGetter<AgentKind>, refreshKey?: MaybeRefOrGetter<unknown>) {
  const settings = useSettings();
  const items = shallowRef<Checkpoint[]>([]);
  const busy = shallowRef(false);
  const error = shallowRef<string | null>(null);
  const available = computed(() => toValue(kind) !== 'claude' || settings.values.value['daemon.checkpointsClaude'] === true || items.value.length > 0);

  async function load() {
    const id = toValue(agent);
    try {
      const next = await listCheckpoints(id);
      if (id === toValue(agent)) items.value = next;
    } catch (failure) {
      error.value = describeError(failure);
    }
  }

  async function restore(number: number): Promise<boolean> {
    if (busy.value) return false;
    busy.value = true;
    error.value = null;
    try {
      await restoreCheckpoint(toValue(agent), number);
      await load();
      return true;
    } catch (failure) {
      error.value = describeError(failure);
      return false;
    } finally {
      busy.value = false;
    }
  }

  watch(
    () => toValue(agent),
    () => {
      items.value = [];
      error.value = null;
      void load();
    },
    { immediate: true },
  );
  watch(
    () => toValue(refreshKey),
    () => void load(),
  );

  return { items, busy, error, available, load, restore };
}
