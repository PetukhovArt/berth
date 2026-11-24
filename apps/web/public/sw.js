const ACTION_URL = '/__berth/push/action';

function openApp() {
  return self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windows) => {
    const open = windows[0];
    return open ? open.focus() : self.clients.openWindow('/');
  });
}

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

self.addEventListener('push', (event) => {
  const data = event.data ? event.data.json() : {};
  const actions = Array.isArray(data.actions) ? data.actions : [];
  const room = typeof Notification !== 'undefined' && Notification.maxActions ? Notification.maxActions : actions.length;
  const shown = actions.length <= room ? actions : actions.filter((entry) => entry.action === 'allow' || entry.action === 'deny').slice(0, room);
  event.waitUntil(
    self.registration.showNotification(data.title || 'Berth', {
      body: data.body,
      tag: data.tag,
      renotify: true,
      actions: shown.map(({ action, title }) => ({ action, title })),
      data: { session: data.session, actions, failed: data.failed, tag: data.tag, openApp: data.openApp },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const { session, actions = [], failed, tag, openApp: openLabel } = event.notification.data || {};
  const chosen = actions.find((entry) => entry.action === event.action);
  if (!chosen || typeof chosen.data !== 'string') {
    event.waitUntil(openApp());
    return;
  }
  event.waitUntil(
    fetch(ACTION_URL, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session, data: chosen.data }),
    })
      .then((response) => {
        if (!response.ok) throw new Error(String(response.status));
        if (chosen.done) return self.registration.showNotification('Berth', { body: chosen.done, tag, actions: openLabel ? [{ action: 'open', title: openLabel }] : [] });
      })
      .catch(() => self.registration.showNotification('Berth', { body: failed, tag })),
  );
});
