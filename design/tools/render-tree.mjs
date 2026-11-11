// Renders a tree file to a PNG overview: links as lines, nodes as squares in their branch colour.
// Usage: node design/tools/render-tree.mjs [tree.json] [out.png] [size] [cx cy span]
// With cx, cy and span it renders only the square of world units centred on (cx, cy).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Canvas, hex } from './png.mjs';

const MOD = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [treeArg, outArg, sizeArg, cxArg, cyArg, spanArg] = process.argv.slice(2);
const TREE = treeArg ?? path.join(MOD, 'design/out/tree-preview.json');
const OUT = outArg ?? path.join(MOD, 'design/out/tree-preview.png');
const SIZE = Number(sizeArg ?? 2000);

const NODE_SIZE = 22;
const SCALE = { start: 1.3, big: 1.3, upgrade: 0.8, small: 0.65 };
const BACKGROUND = hex('#14161c');
const LINK = [120, 126, 140, 255];
const GOLD = hex('#f2c14e');
const FALLBACK = ['#e8575a', '#e8a33d', '#d6d24a', '#7fcf55', '#47c9a0', '#4ab4e0', '#6b7cf0', '#b46ff0'];

const tree = JSON.parse(fs.readFileSync(TREE, 'utf8'));
const nodes = tree.nodes;
const extent = Math.max(...nodes.map((n) => Math.max(Math.abs(n.x), Math.abs(n.y)))) + 40;
const cx = Number(cxArg ?? 0);
const cy = Number(cyArg ?? 0);
const span = Number(spanArg ?? extent * 2);
const zoom = SIZE / span;
const toPixel = (x, y) => [Math.round((x - cx) * zoom + SIZE / 2), Math.round((y - cy) * zoom + SIZE / 2)];

const branchIds = [...new Set(nodes.map((n) => n.branch).filter(Boolean))];
const colourOf = (branch) => {
  const entry = tree.branches?.[branch];
  if (entry?.colour) return hex(entry.colour);
  return hex(FALLBACK[branchIds.indexOf(branch) % FALLBACK.length] ?? '#cccccc');
};

const canvas = new Canvas(SIZE, SIZE);
canvas.rect(0, 0, SIZE, SIZE, BACKGROUND);

function line([x0, y0], [x1, y1], colour, width) {
  const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
  const half = Math.floor(width / 2);
  for (let i = 0; i <= steps; i++) {
    const x = Math.round(x0 + ((x1 - x0) * i) / steps);
    const y = Math.round(y0 + ((y1 - y0) * i) / steps);
    canvas.rect(x - half, y - half, width, width, colour);
  }
}

const byId = new Map(nodes.map((n) => [n.id, n]));
const linkWidth = Math.max(1, Math.round(2 * zoom));
for (const node of nodes) {
  for (const to of node.links ?? []) {
    const other = byId.get(to);
    if (other) line(toPixel(node.x, node.y), toPixel(other.x, other.y), LINK, linkWidth);
  }
}
for (const node of nodes) {
  const side = Math.max(2, Math.round(NODE_SIZE * (SCALE[node.kind] ?? 0.65) * zoom));
  const [x, y] = toPixel(node.x, node.y);
  const left = x - Math.floor(side / 2);
  const top = y - Math.floor(side / 2);
  const fill = node.kind === 'start' ? GOLD : node.branch ? colourOf(node.branch) : hex('#ffffff');
  if (node.kind === 'big') canvas.rect(left - 1, top - 1, side + 2, side + 2, GOLD);
  canvas.rect(left, top, side, side, fill);
  if ((node.ranks ?? 1) > 1) canvas.rect(x - 1, y - 1, 3, 3, [255, 255, 255, 255]);
}

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, canvas.png());
console.log(`wrote ${OUT}: ${SIZE}px, ${zoom.toFixed(3)} px per unit`);
