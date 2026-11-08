import type { Terminal } from '@xterm/xterm';

type RenderCore = { _core: { _renderService: { clear: () => void; dimensions: { css: { cell: { width: number; height: number } } } } } };

export function fitTerminal(term: Terminal, element: HTMLElement): void {
  const render = (term as unknown as RenderCore)._core._renderService;
  const { width, height } = render.dimensions.css.cell;
  if (!width || !height || !element.clientWidth || !element.clientHeight) return;
  const cols = Math.max(2, Math.floor(element.clientWidth / width));
  const rows = Math.max(1, Math.floor(element.clientHeight / height));
  if (cols === term.cols && rows === term.rows) return;
  render.clear();
  term.resize(cols, rows);
}
