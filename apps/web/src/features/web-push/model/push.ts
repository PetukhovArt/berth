import { shallowRef } from 'vue';
import { isDesktop, remoteFetch } from '../../../bridge';

export type PushState = 'unsupported' | 'denied' | 'off' | 'on';

const BASE = '/__berth/push';

export const pushState = shallowRef<PushState>('off');

const supported = () => !isDesktop && 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window && window.isSecureContext;

function post(op: string, body: unknown): Promise<Response> {
  return remoteFetch(`${BASE}/${op}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
}

function decodeKey(text: string): Uint8Array<ArrayBuffer> {
  const padded = text.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(text.length / 4) * 4, '=');
  return Uint8Array.from(atob(padded), (char) => char.charCodeAt(0));
}

async function registration(): Promise<ServiceWorkerRegistration> {
  await navigator.serviceWorker.register('/sw.js');
  return navigator.serviceWorker.ready;
}

export async function refreshPushState(): Promise<void> {
  if (!supported()) {
    pushState.value = 'unsupported';
    return;
  }
  if (Notification.permission === 'denied') {
    pushState.value = 'denied';
    return;
  }
  const existing = await (await registration()).pushManager.getSubscription();
  pushState.value = existing && Notification.permission === 'granted' ? 'on' : 'off';
}

export async function enablePush(): Promise<void> {
  if (!supported()) return;
  const permission = await Notification.requestPermission();
  if (permission !== 'granted') {
    pushState.value = permission === 'denied' ? 'denied' : 'off';
    return;
  }
  const keyResponse = await remoteFetch(`${BASE}/key`);
  if (!keyResponse.ok) throw new Error(`push key ${keyResponse.status}`);
  const key = decodeKey(await keyResponse.text());
  const worker = await registration();
  const subscription = (await worker.pushManager.getSubscription()) ?? (await worker.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key }));
  const saved = await post('subscribe', subscription.toJSON());
  if (!saved.ok) throw new Error(`push subscribe ${saved.status}`);
  pushState.value = 'on';
}

export async function disablePush(): Promise<void> {
  if (!supported()) return;
  const subscription = await (await registration()).pushManager.getSubscription();
  if (subscription) {
    await post('unsubscribe', { endpoint: subscription.endpoint });
    await subscription.unsubscribe();
  }
  pushState.value = 'off';
}
