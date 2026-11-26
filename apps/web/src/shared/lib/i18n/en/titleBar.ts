export const titleBar = {
  workspace: {
    label: 'Workspace: {name}',
    all: 'All workspaces',
    menu: 'Switch workspace',
  },
  connectors: {
    unavailable: { one: '{count} connector unavailable', other: '{count} connectors unavailable' },
    tooltip: 'Not responding: {names}',
    title: 'Connectors · {count} not responding',
    since: 'since {time}',
    retry: 'Retry',
    configure: 'Configure',
    vpnHint: 'Hosts are not responding: check the VPN.',
  },
} as const;
