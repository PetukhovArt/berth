import { watch } from 'vue';
import { useAiAccounts, type AiKind } from '../../../shared/api/connectors/ai';
import type { Provider } from '../../../shared/api/providers';

export type KindState = 'ready' | 'missing' | 'signedOut';

const ACCOUNT_OF: Readonly<Record<string, AiKind>> = { claude: 'claude', codex: 'codex' };

export function useKindStates() {
  const ai = useAiAccounts();
  void ai.load();

  const rechecked = new Set<AiKind>();
  watch(
    ai.accounts,
    (accounts) => {
      for (const account of accounts) {
        if ((account.status !== 'signed_out' && account.status !== 'unchecked') || rechecked.has(account.kind) || !Object.values(ACCOUNT_OF).includes(account.kind)) continue;
        rechecked.add(account.kind);
        void ai.recheck(account.kind);
      }
    },
    { immediate: true },
  );

  function stateOf(provider: Provider): KindState {
    if (provider.acp) return provider.found === false ? 'missing' : 'ready';
    const kind = ACCOUNT_OF[provider.id];
    const status = kind ? ai.accounts.value.find((account) => account.kind === kind)?.status : undefined;
    if (status === 'not_installed') return 'missing';
    if (status === 'signed_out') return 'signedOut';
    return 'ready';
  }

  function signIn(provider: Provider) {
    const kind = ACCOUNT_OF[provider.id];
    if (kind) void ai.login(kind);
  }

  return { stateOf, signIn };
}
