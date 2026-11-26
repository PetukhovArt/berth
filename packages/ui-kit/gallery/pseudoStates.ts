const REPLACEMENTS: [RegExp, string][] = [
  [/:hover/g, '.pseudo-hover'],
  [/:focus-visible/g, '.pseudo-focus'],
  [/:focus(?![-\w])/g, '.pseudo-focus'],
  [/:active/g, '.pseudo-active'],
];

const PSEUDO = /:(?:hover|focus|active)/;
const seen = new WeakSet<CSSStyleSheet>();

function mirror(list: CSSRuleList, target: CSSStyleSheet | CSSGroupingRule): void {
  const copies: [number, string][] = [];
  for (let index = 0; index < list.length; index++) {
    const rule = list[index];
    if (rule instanceof CSSStyleRule) {
      if (!PSEUDO.test(rule.selectorText)) continue;
      const selector = REPLACEMENTS.reduce((text, [pattern, to]) => text.replace(pattern, to), rule.selectorText);
      copies.push([index + 1, `${selector} { ${rule.style.cssText} }`]);
    } else if (rule instanceof CSSGroupingRule) mirror(rule.cssRules, rule);
  }
  for (const [index, text] of copies.reverse()) {
    try {
      target.insertRule(text, index);
    } catch {
      continue;
    }
  }
}

function mirrorAll(): void {
  for (const sheet of Array.from(document.styleSheets)) {
    if (seen.has(sheet)) continue;
    seen.add(sheet);
    try {
      mirror(sheet.cssRules, sheet);
    } catch {
      continue;
    }
  }
}

export function installPseudoStates(): void {
  let queued = false;
  const schedule = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      mirrorAll();
    });
  };
  new MutationObserver(schedule).observe(document.head, { childList: true, subtree: true, characterData: true });
  mirrorAll();
}
