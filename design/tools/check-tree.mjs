// Checks a tree file's layout and effects.
// Usage: node design/tools/check-tree.mjs [tree.json] [reference.json]
// Reports link crossings, links passing over a node they do not connect, overlapping nodes, the
// closest distance between node centres, the node count, and the effect totals per key (value
// times ranks) compared with the reference, data/tree.json by default. Exits 1 on any problem.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const MOD = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const TREE = process.argv[2] ?? path.join(MOD, 'design/out/tree-preview.json');
const REFERENCE = process.argv[3] ?? path.join(MOD, 'mixins/path_of_craft_tree/data/tree.json');

// Node sides from view.rs (NODE_SIZE 22) and ui.rs (node_scale); the radius is half the diagonal.
const NODE_SIZE = 22;
const SCALE = { start: 1.3, big: 1.3, upgrade: 0.8, small: 0.65 };
const radius = (node) => ((NODE_SIZE * (SCALE[node.kind] ?? 0.65)) / 2) * Math.SQRT2;
const TOTAL_EPSILON = 1e-9;

const load = (file) => JSON.parse(fs.readFileSync(file, 'utf8'));

function edges(nodes) {
  const ids = new Set(nodes.map((n) => n.id));
  const seen = new Set();
  const list = [];
  const broken = [];
  for (const node of nodes) {
    for (const to of node.links ?? []) {
      if (!ids.has(to)) broken.push(`${node.id}->${to}`);
      const key = node.id < to ? `${node.id}-${to}` : `${to}-${node.id}`;
      if (seen.has(key) || !ids.has(to)) continue;
      seen.add(key);
      list.push([node.id, to]);
    }
  }
  return { list, broken };
}

const sub = (a, b) => ({ x: a.x - b.x, y: a.y - b.y });
const cross = (a, b) => a.x * b.y - a.y * b.x;

function pointSegment(p, a, b) {
  const ab = sub(b, a);
  const len2 = ab.x * ab.x + ab.y * ab.y || 1e-9;
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2));
  return Math.hypot(p.x - (a.x + ab.x * t), p.y - (a.y + ab.y * t));
}

function segmentsTouch(a, b, c, d) {
  const d1 = cross(sub(d, c), sub(a, c));
  const d2 = cross(sub(d, c), sub(b, c));
  const d3 = cross(sub(b, a), sub(c, a));
  const d4 = cross(sub(b, a), sub(d, a));
  if (d1 * d2 < 0 && d3 * d4 < 0) return true;
  const eps = 1e-6;
  return (
    pointSegment(a, c, d) < eps || pointSegment(b, c, d) < eps ||
    pointSegment(c, a, b) < eps || pointSegment(d, a, b) < eps
  );
}

/// Pairs of links that share no node and touch, and pairs that share a node and overlap.
function crossings(list, at) {
  const found = [];
  for (let i = 0; i < list.length; i++) {
    const [a, b] = list[i];
    for (let j = i + 1; j < list.length; j++) {
      const [c, d] = list[j];
      const shared = [a, b].filter((x) => x === c || x === d);
      if (shared.length === 0) {
        if (segmentsTouch(at.get(a), at.get(b), at.get(c), at.get(d))) found.push([list[i], list[j]]);
      } else if (shared.length === 1) {
        // Links from one node overlap when the far end of one lies on the other.
        const s = shared[0];
        const p = a === s ? b : a;
        const q = c === s ? d : c;
        const S = at.get(s);
        if (pointSegment(at.get(p), S, at.get(q)) < 1e-6 || pointSegment(at.get(q), S, at.get(p)) < 1e-6) {
          found.push([list[i], list[j]]);
        }
      }
    }
  }
  return found;
}

function linksOverNodes(list, nodes, at) {
  const found = [];
  for (const [a, b] of list) {
    const A = at.get(a);
    const B = at.get(b);
    const minX = Math.min(A.x, B.x) - 25;
    const maxX = Math.max(A.x, B.x) + 25;
    const minY = Math.min(A.y, B.y) - 25;
    const maxY = Math.max(A.y, B.y) + 25;
    for (const node of nodes) {
      if (node.id === a || node.id === b) continue;
      if (node.x < minX || node.x > maxX || node.y < minY || node.y > maxY) continue;
      const distance = pointSegment(node, A, B);
      if (distance < radius(node)) found.push({ link: [a, b], node: node.id, distance: +distance.toFixed(2) });
    }
  }
  return found;
}

function spacing(nodes) {
  let closest = { distance: Infinity, pair: null };
  const overlaps = [];
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const distance = Math.hypot(nodes[i].x - nodes[j].x, nodes[i].y - nodes[j].y);
      if (distance < closest.distance) closest = { distance, pair: [nodes[i].id, nodes[j].id] };
      if (distance < radius(nodes[i]) + radius(nodes[j])) overlaps.push([nodes[i].id, nodes[j].id, +distance.toFixed(2)]);
    }
  }
  return { closest, overlaps };
}

function totals(nodes) {
  const sum = new Map();
  for (const node of nodes) {
    for (const [key, value] of Object.entries(node.effects ?? {})) {
      sum.set(key, (sum.get(key) ?? 0) + value * (node.ranks ?? 1));
    }
  }
  return sum;
}

function compareTotals(mine, theirs) {
  const differences = [];
  for (const key of new Set([...mine.keys(), ...theirs.keys()])) {
    const a = mine.get(key) ?? 0;
    const b = theirs.get(key) ?? 0;
    if (Math.abs(a - b) > TOTAL_EPSILON) differences.push({ key, tree: a, reference: b });
  }
  return differences;
}

const tree = load(TREE);
const reference = load(REFERENCE);
const nodes = tree.nodes;
const at = new Map(nodes.map((n) => [n.id, n]));
const { list, broken } = edges(nodes);
const crossed = crossings(list, at);
const over = linksOverNodes(list, nodes, at);
const { closest, overlaps } = spacing(nodes);
const mine = totals(nodes);
const differences = compareTotals(mine, totals(reference.nodes));
const ranked = nodes.filter((n) => (n.ranks ?? 1) > 1);

console.log(`tree: ${path.relative(process.cwd(), TREE)}`);
console.log(`nodes: ${nodes.length} (reference ${reference.nodes.length}), links: ${list.length}, ranked nodes: ${ranked.length}`);
console.log(`start id: ${tree.start}, broken links: ${broken.length}`);
console.log(`link crossings: ${crossed.length}`);
for (const pair of crossed.slice(0, 10)) console.log(`  ${JSON.stringify(pair)}`);
console.log(`links over a node they do not connect: ${over.length}`);
for (const item of over.slice(0, 10)) console.log(`  ${JSON.stringify(item)}`);
console.log(`overlapping nodes: ${overlaps.length}`);
for (const item of overlaps.slice(0, 10)) console.log(`  ${JSON.stringify(item)}`);
console.log(`closest node centres: ${closest.distance.toFixed(2)} (${closest.pair})`);
console.log(`effect keys: ${mine.size}, totals differing from the reference: ${differences.length}`);
for (const item of differences.slice(0, 20)) console.log(`  ${JSON.stringify(item)}`);

const problems = broken.length + crossed.length + over.length + overlaps.length + differences.length;
console.log(problems === 0 ? 'OK' : `PROBLEMS: ${problems}`);
process.exitCode = problems === 0 ? 0 : 1;
