export type HoldToTalkIssue = 'denied' | 'short' | 'empty' | 'failed' | 'offline' | 'loading' | 'phrase';

export type TakeOptions = { handsFree: boolean; wakePhrase: string };
export type TakeEvents = { woke: () => void; speech: () => void; interim: (text: string) => void; end: (text: string, issue: HoldToTalkIssue | null) => void };
export type Take = { stop: () => void; abort: () => void };
export type DictationEngine = { supported: boolean; open: (options: TakeOptions, events: TakeEvents) => Take };
