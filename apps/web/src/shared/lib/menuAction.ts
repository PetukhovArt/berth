export type MenuAction = { label: string; testid: string; disabled?: boolean; run: (anchor: HTMLElement) => void };

type Anchored = { toggle: (event: Event) => void };

export function toggleAt(popover: Anchored | null | undefined, anchor: HTMLElement) {
  popover?.toggle({ currentTarget: anchor } as unknown as Event);
}
