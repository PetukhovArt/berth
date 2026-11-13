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
  event.waitUntil(
    self.registration.showNotification(data.title || 'Berth', {
      body: data.body,
      tag: data.tag,
      renotify: true,
      actions: actions.map(({ action, title }) => ({ action, title })),
      data: { session: data.session, actions, failed: data.failed, tag: data.tag },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const { session, actions = [], failed, tag } = event.notification.data || {};
  const chosen = actions.find((entry) => entry.action === event.action);
  if (!chosen) {
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
      })
      .catch(() => self.registration.showNotification('Berth', { body: failed, tag })),
  );
});
