// Packs the tree window's pixel art for the game: node frames, icons, rank badges, branch banners
// and the stone backdrop, drawn by the approved preview's own code (design/preview), into
// mixins/path_of_craft_tree/data/atlas.png with its sprite table atlas.json.
//   node design/tools/icons.mjs all && node design/tools/game-atlas.mjs   (from the repository root)
// The game uploads the picture into its 1024 x 1024 mod image slot; the rows under the packed
// sprites are left free for the colour zones the game computes from the tree it receives.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { Canvas } from './png.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const at = (...parts) => join(ROOT, ...parts);
const SIZE = 1024;
const GAP = 1;
const KINDS = ['small', 'upgrade', 'big', 'start'];
const STATES = ['locked', 'available', 'partial', 'learned', 'off'];
const VARIANTS = { n: 'learned', l: 'locked', o: 'off' };
const OUT_PNG = at('mixins/path_of_craft_tree/data/atlas.png');
const OUT_JSON = at('mixins/path_of_craft_tree/data/atlas.json');

/** A browser just big enough for the preview's sprite code: canvases keep their pixels. */
function browser() {
  const context2d = (canvas) => ({
    imageSmoothingEnabled: false,
    font: '',
    measureText: () => ({ width: 0 }),
    createImageData: (w, h) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
    putImageData: (img) => {
      canvas.image = img;
    },
  });
  const document = {
    createElement: () => {
      const canvas = { width: 0, height: 0 };
      canvas.getContext = () => context2d(canvas);
      return canvas;
    },
  };
  const window = {};
  const sandbox = vm.createContext({ window, document, Math, Map, Set, Array, Uint32Array, Uint8Array, Uint8ClampedArray });
  for (const file of ['design/out/icons.js', 'design/preview/pixels.js', 'design/preview/sprites.js']) {
    vm.runInContext(readFileSync(at(file), 'utf8'), sandbox, { filename: file });
  }
  return window.PCT;
}

const PCT = browser();
const { full, compact, iconFor, badge, pips, crop, PAD } = PCT.parts;
const tree = JSON.parse(readFileSync(at('mixins/path_of_craft_tree/data/tree.json'), 'utf8'));

/** A sprite to pack: its pixels (little-endian RGBA words) and the anchor inside it. */
const sprites = [];
const add = (name, pix, cx = 0, cy = 0) => sprites.push({ name, w: pix.w, h: pix.h, d: pix.d, cx, cy });

for (const kind of KINDS) {
  for (const state of STATES) {
    for (const [prefix, shapes] of [['frame', full], ['cframe', compact]]) {
      const p = new PCT.Pix(52, 52);
      const shape = shapes[kind](p, PCT.STATES[state], state);
      const cut = crop(p, PAD + shape.size / 2);
      add(`${prefix}:${kind}:${state}`, cut.pix, cut.cx, cut.cy);
    }
  }
}

const iconIds = new Set(['?']);
for (const node of tree.nodes) iconIds.add(node.icon);
for (const id of iconIds) {
  for (const [variant, state] of Object.entries(VARIANTS)) {
    const big = iconFor(id === '?' ? 'none' : id, state, false);
    const small = iconFor(id === '?' ? 'none' : id, state, true);
    add(`icon:${id}:${variant}`, big, 8, 8);
    add(`cicon:${id}:${variant}`, small, 4, 4);
  }
}
const missing = [...iconIds].filter((id) => id !== '?' && !PCT.hasIcon(id));

const rankCounts = new Set(tree.nodes.map((n) => n.ranks ?? 1).filter((r) => r > 1));
for (const ranks of rankCounts) {
  for (let rank = 0; rank <= ranks; rank++) {
    const label = `${rank}/${ranks}`;
    const b = new PCT.Pix(PCT.tinyWidth(label) + 4, 9);
    badge(b, b.w - 1, 0, rank, ranks);
    add(`badge:${label}`, b);
    const p = new PCT.Pix(ranks * 3 + 1, 4);
    pips(p, p.w - 1, 0, rank, ranks);
    add(`pips:${label}`, p);
  }
}

for (const [id, branch] of Object.entries(tree.branches)) {
  const banner = PCT.bannerSprite(branch.icon ?? `branch:${id}`, 1, branch.colour);
  const img = banner.canvas.image;
  const pix = new PCT.Pix(img.width, img.height);
  pix.d.set(new Uint32Array(img.data.buffer));
  add(`banner:${id}`, pix, banner.cx, banner.cy);
}

PCT.backdropTiles().forEach((tile, k) => {
  const pix = new PCT.Pix(PCT.TILE, PCT.TILE);
  pix.d.set(tile);
  add(`stone:${k}`, pix);
});

// Shelf packing, tallest first, with a transparent gap so stretched sprites never bleed.
sprites.sort((a, b) => b.h - a.h || b.w - a.w);
let [x, y, shelf] = [0, 0, 0];
const table = {};
for (const s of sprites) {
  if (x + s.w > SIZE) {
    [x, y, shelf] = [0, y + shelf + GAP, 0];
  }
  s.x = x;
  s.y = y;
  table[s.name] = [x, y, s.w, s.h, s.cx, s.cy];
  x += s.w + GAP;
  shelf = Math.max(shelf, s.h);
}
const used = y + shelf + GAP;
if (used >= SIZE) throw new Error(`the sprites need ${used} rows of ${SIZE}`);

const canvas = new Canvas(SIZE, used);
for (const s of sprites) {
  const bytes = Buffer.from(s.d.buffer, s.d.byteOffset, s.d.byteLength);
  for (let j = 0; j < s.h; j++) bytes.copy(canvas.data, ((s.y + j) * SIZE + s.x) * 4, j * s.w * 4, (j + 1) * s.w * 4);
}
writeFileSync(OUT_PNG, canvas.png());
writeFileSync(OUT_JSON, JSON.stringify({ size: SIZE, free: [0, used, SIZE, SIZE - used], sprites: table }));
console.log(`atlas: ${sprites.length} sprites in ${SIZE}x${used}, ${SIZE - used} rows free; icons missing: ${missing.length}`);
for (const id of missing.slice(0, 20)) console.log(`  ${id}`);
