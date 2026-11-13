export type AiKind = 'claude' | 'codex' | 'gemini' | 'opencode' | 'anthropic_api' | 'openai_api' | 'openrouter' | 'grok';
export type AiStatus = 'signed_in' | 'signed_out' | 'not_installed' | 'rejected' | 'unreachable' | 'error' | 'unchecked';

export type AiView = {
  kind: AiKind;
  label: string;
  method: 'cli' | 'key';
  status: AiStatus;
  account: string | null;
  plan: string | null;
  detail: string | null;
  version: string | null;
  expires_at: string | null;
  command: string | null;
};

export type ConnectorKind = 'github' | 'gitlab' | 'jira' | 'confluence';
export type ConnectorAuth = 'cli' | 'token';
export type ConnectorStatus = 'ok' | 'error' | 'unreachable' | 'unchecked';

export type Person = { name: string; username: string; avatar_url: string | null };

export type ConnectorView = {
  id: string;
  kind: ConnectorKind;
  label: string;
  base_url: string;
  auth: ConnectorAuth;
  account: Person | null;
  status: ConnectorStatus;
  error: string | null;
};

export type ConnectorsChanged = { connectors: ConnectorView[]; ai: AiView[] };
