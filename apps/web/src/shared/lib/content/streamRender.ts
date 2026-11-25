import { renderRich, type Rendered } from './render';
import { splitMarkdownBlocks } from './markdownBlocks';

const DIAGRAM_MARK = /data-rc-mermaid="(\d+)"/g;

export function createStreamRenderer(): (markdown: string, breaks: boolean, epoch: number) => Rendered {
  let cache = new Map<string, Rendered>();
  let seen = 0;
  return (markdown, breaks, epoch) => {
    const blocks = splitMarkdownBlocks(markdown);
    if (!blocks) return renderRich({ markdown, breaks });
    if (epoch !== seen) {
      cache = new Map();
      seen = epoch;
    }
    const next = new Map<string, Rendered>();
    const diagrams: string[] = [];
    let html = '';
    for (const block of blocks) {
      const rendered = next.get(block) ?? cache.get(block) ?? renderRich({ markdown: block, breaks });
      next.set(block, rendered);
      const base = diagrams.length;
      html += base ? rendered.html.replace(DIAGRAM_MARK, (_, index: string) => `data-rc-mermaid="${Number(index) + base}"`) : rendered.html;
      diagrams.push(...rendered.diagrams);
    }
    cache = next;
    return { html, diagrams };
  };
}
