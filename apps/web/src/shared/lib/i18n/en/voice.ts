export const voice = {
  category: 'Voice',
  action: {
    pushToTalk: 'Push to talk: hold to dictate a prompt to the Session in view',
    wakeWord: 'Wake word: listen for the wake phrase for the Session in view',
  },
  holding: 'Listening for {name}… release to send',
  wake: 'Wake word on: say "{phrase}" to talk to {name}',
  armed: 'Listening for your prompt to {name}…',
  off: 'Turn off',
  sent: 'Sent to {name}: {text}',
  short: 'Hold the key while you talk',
  noSession: 'Open a Session first',
  gone: 'The Session for the wake word is gone',
  unsupported: 'Speech recognition is not available in this window',
  setting: {
    label: 'Wake phrase',
    description: 'Words that start a spoken prompt while the wake word is on. Turn the wake word on for the Session in view from the command palette.',
  },
} as const;
