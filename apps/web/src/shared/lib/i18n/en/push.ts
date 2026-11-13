export const push = {
  title: 'Notifications',
  unsupported: 'This browser cannot receive push notifications. Open Berth over HTTPS (Tailscale HTTPS) and add it to the home screen.',
  denied: 'Notifications are blocked for this site in the browser settings.',
  off: 'Get a notification here when an Agent is waiting for you.',
  on: 'This device gets a notification when an Agent is waiting for you.',
  enable: 'Enable notifications',
  disable: 'Disable notifications',
  allow: 'Allow',
  deny: 'Deny',
  failed: 'Could not reach Berth. Open the app to answer.',
} as const;
