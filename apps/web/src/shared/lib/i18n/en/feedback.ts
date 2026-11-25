export const feedback = {
  permission: '{tool} needs permission',
  orTerminal: 'or answer in the terminal',
  allow: 'Allow',
  always: 'Always',
  alwaysTitle: "Allow, and don't ask again for this kind of call",
  deny: 'Deny',
  place: 'in {place}',
  allowed: 'Allowed',
  denied: 'Denied',
  answered: 'Answered “{answer}”',
  asks: '{agent} asks a question',
  queue: {
    sendsWhenFinished: 'Sends when the agent finishes',
    remove: 'Remove',
  },
} as const;
