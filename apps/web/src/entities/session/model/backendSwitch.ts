import type { Provider } from '../../../shared/api/providers';
import { isChat, type AgentKind, type Backend } from './berth';
import { isHarness } from './external';

type Switchable = { kind: AgentKind; backend?: Backend; acp_provider?: string | null; archived: boolean; ended_at: number };

export function otherBackend(agent: Switchable, providers: readonly Provider[]): Backend | null {
  if (!isHarness(agent.kind) || agent.archived || agent.ended_at) return null;
  if (isChat(agent)) return 'terminal';
  const chat = agent.acp_provider ?? agent.kind;
  return providers.some((provider) => provider.acp && provider.enabled && provider.id === chat) ? 'chat' : null;
}
