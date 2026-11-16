import { invoke } from '../../../bridge';

type Device = { scope: string | { session: string } };
type Pairing = { url: string };

export async function isShared(agent: string): Promise<boolean> {
  const status = await invoke<{ devices: Device[] }>('remote_status');
  return status.devices.some((device) => typeof device.scope === 'object' && device.scope.session === agent);
}

export async function shareLink(agent: string, name: string): Promise<string> {
  const pairing = await invoke<Pairing>('remote_share_session', { agent, name });
  return pairing.url;
}

export function stopSharing(agent: string): Promise<void> {
  return invoke<void>('remote_unshare_session', { agent });
}
