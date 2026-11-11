// Validates the pixel icons and renders review sheets, the game atlas and the preview's icon data.
//   node design/tools/icons.mjs [gather|farm|craft|roam|all]   (run from the repository root)
// Writes design/out/sheet-<sector>.png (4x, numbered; the key is in sheet-<sector>.txt),
// design/out/atlas.png (16 px cells), design/out/atlas.json (id -> [x, y]) and design/out/icons.js.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { Canvas, hex, number } from './png.mjs';

const SIZE = 16;
const SCALE = 4;
const COLUMNS = 8;
const CELL = SIZE * SCALE + 12;
const LABEL = 14;
const ATLAS_WIDTH = 1024;
const MIN_PIXELS = 40;
const SECTORS = ['gather', 'farm', 'craft', 'roam'];
const want = process.argv[2] ?? 'all';

const palette = JSON.parse(readFileSync('design/icons/palette.json', 'utf8')).colours;
const colours = Object.fromEntries(Object.entries(palette).map(([c, [h]]) => [c, hex(h)]));
const list = JSON.parse(readFileSync('design/icon-list.json', 'utf8'));
const icons = {};
for (const sector of SECTORS) {
  const file = `design/icons/${sector}.json`;
  if (existsSync(file)) Object.assign(icons, JSON.parse(readFileSync(file, 'utf8')));
}

const problems = [];
for (const [id, rows] of Object.entries(icons)) {
  if (!list.some((i) => i.id === id)) problems.push(`${id}: not in icon-list.json`);
  if (!Array.isArray(rows) || rows.length !== SIZE) {
    problems.push(`${id}: needs ${SIZE} rows`);
    continue;
  }
  rows.forEach((row, y) => {
    if ([...row].length !== SIZE) problems.push(`${id}: row ${y} has ${[...row].length} pixels`);
    for (const ch of row) if (ch !== '.' && !colours[ch]) problems.push(`${id}: unknown colour '${ch}' in row ${y}`);
  });
  const filled = rows.join('').replace(/\./g, '').length;
  if (filled < MIN_PIXELS) problems.push(`${id}: only ${filled} pixels drawn`);
}

const draw = (canvas, rows, ox, oy, scale) =>
  rows.forEach((row, y) =>
    [...row].forEach((ch, x) => {
      if (ch !== '.' && colours[ch]) canvas.rect(ox + x * scale, oy + y * scale, scale, scale, colours[ch]);
    }),
  );

mkdirSync('design/out', { recursive: true });
for (const sector of SECTORS.filter((s) => want === 'all' || want === s)) {
  const entries = list.filter((i) => i.sector === sector);
  const sheet = new Canvas(COLUMNS * CELL, Math.ceil(entries.length / COLUMNS) * (CELL + LABEL));
  sheet.rect(0, 0, sheet.width, sheet.height, [40, 40, 48, 255]);
  const key = [];
  entries.forEach((entry, n) => {
    const x = (n % COLUMNS) * CELL + 6;
    const y = Math.floor(n / COLUMNS) * (CELL + LABEL) + LABEL;
    for (let j = 0; j < SIZE; j++) {
      for (let i = 0; i < SIZE; i++) {
        const shade = (i + j) % 2 ? [70, 70, 78, 255] : [58, 58, 66, 255];
        sheet.rect(x + i * SCALE, y + j * SCALE, SCALE, SCALE, shade);
      }
    }
    const rows = icons[entry.id];
    if (rows) draw(sheet, rows, x, y, SCALE);
    else sheet.rect(x + 20, y + 20, 24, 24, [200, 40, 40, 255]);
    number(sheet, x, y - 12, n + 1, rows ? [230, 230, 230, 255] : [255, 90, 90, 255]);
    key.push(`${n + 1}\t${rows ? 'ok' : 'MISSING'}\t${entry.id}\t${entry.name}`);
  });
  writeFileSync(`design/out/sheet-${sector}.png`, sheet.png());
  writeFileSync(`design/out/sheet-${sector}.txt`, key.join('\n'));
}

const badges = JSON.parse(readFileSync('design/icons/badges.json', 'utf8'));
const badgeSheet = new Canvas(Object.keys(badges).length * 44 + 4, 48);
badgeSheet.rect(0, 0, badgeSheet.width, badgeSheet.height, [40, 40, 48, 255]);
Object.values(badges).forEach((rows, n) => draw(badgeSheet, rows, 4 + n * 44, 10, 5));
writeFileSync('design/out/sheet-badges.png', badgeSheet.png());
writeFileSync('design/out/sheet-badges.txt', Object.keys(badges).map((b, n) => `${n + 1}\t${b}`).join('\n'));

const ids = list.map((i) => i.id).filter((id) => icons[id]);
const perRow = ATLAS_WIDTH / SIZE;
const atlas = new Canvas(ATLAS_WIDTH, Math.max(SIZE, Math.ceil(ids.length / perRow) * SIZE));
const places = {};
ids.forEach((id, n) => {
  const [x, y] = [(n % perRow) * SIZE, Math.floor(n / perRow) * SIZE];
  draw(atlas, icons[id], x, y, 1);
  places[id] = [x, y];
});
writeFileSync('design/out/atlas.png', atlas.png());
writeFileSync('design/out/atlas.json', JSON.stringify(places));
const flat = Object.fromEntries(Object.entries(palette).map(([c, [h]]) => [c, h]));
writeFileSync('design/out/icons.js', `window.PCT_ICONS = ${JSON.stringify({ palette: flat, icons })};\n`);

const missing = list.filter((i) => !icons[i.id]).length;
console.log(`icons drawn ${Object.keys(icons).length}/${list.length}, missing ${missing}, problems ${problems.length}`);
for (const p of problems.slice(0, 40)) console.log('  ' + p);
