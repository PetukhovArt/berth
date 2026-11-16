import { invoke } from '../../../bridge';

type Device = { scope: string | { session: string }; diffs: boolean };
type Pairing = { url: string };
export type ShareState = { shared: boolean; diffs: boolean };

export async function shareState(agent: string): Promise<ShareState> {
  const status = await invoke<{ devices: Device[] }>('remote_status');
  const device = status.devices.find((entry) => typeof entry.scope === 'object' && entry.scope.session === agent);
  return { shared: !!device, diffs: !!device?.diffs };
}

export async function shareLink(agent: string, name: string, diffs: boolean): Promise<string> {
  const pairing = await invoke<Pairing>('remote_share_session', { agent, name, diffs });
  return pairing.url;
}

export function stopSharing(agent: string): Promise<void> {
  return invoke<void>('remote_unshare_session', { agent });
}
