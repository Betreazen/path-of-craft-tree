// Builds design/preview.html: one self-contained page with the tree, the pixel icons and the
// preview code inlined, so it opens straight from disk.
//   node design/tools/build-preview.mjs   (works from any folder)
// Tree: design/out/tree-preview.json when it exists, else mixins/path_of_craft_tree/data/tree.json.
// Icons: design/out/icons.js (regenerate with `node design/tools/icons.mjs all`).
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const at = (...parts) => join(ROOT, ...parts);
const SCRIPTS = ['pixels.js', 'sprites.js', 'tree.js', 'render.js', 'ui.js'];
const ICON_ID = /^(start$|big:|small:|branch:|upgrade:)/;

const treeFile = existsSync(at('design/out/tree-preview.json'))
  ? at('design/out/tree-preview.json')
  : at('mixins/path_of_craft_tree/data/tree.json');
const tree = JSON.parse(readFileSync(treeFile, 'utf8'));
const byId = new Map(tree.nodes.map((n) => [n.id, n]));

/** The pixel icon id of a node, worked out the way design/tools/icon-list.mjs names them. */
const iconId = (n) => {
  if (typeof n.icon === 'string' && ICON_ID.test(n.icon)) return n.icon;
  if (n.kind === 'start') return 'start';
  if (n.kind === 'big') return `big:${n.name}`;
  if (n.kind === 'upgrade') {
    let parent = byId.get(n.links?.[0]);
    const seen = new Set();
    while (parent && parent.kind === 'upgrade' && !seen.has(parent.id)) {
      seen.add(parent.id);
      parent = byId.get(parent.links?.[0]);
    }
    return parent?.kind === 'big' ? `big:${parent.name}` : 'upgrade';
  }
  return `small:${Object.keys(n.effects ?? {})[0] ?? n.name}`;
};

const KEEP = ['id', 'x', 'y', 'kind', 'branch', 'name', 'description', 'effects', 'todo', 'links', 'ranks', 'rank_descriptions'];
const nodes = tree.nodes.map((n) => {
  const out = Object.fromEntries(KEEP.filter((k) => n[k] !== undefined).map((k) => [k, n[k]]));
  out.icon = iconId(n);
  return out;
});
const branches = Object.fromEntries(
  Object.entries(tree.branches).map(([id, b]) => [id, { name: b.name, icon: b.icon ?? `branch:${id}`, colour: b.colour }]),
);
const start = tree.start ?? tree.nodes.find((n) => n.kind === 'start')?.id ?? 0;

const iconsFile = at('design/out/icons.js');
const icons = existsSync(iconsFile) ? readFileSync(iconsFile, 'utf8') : 'window.PCT_ICONS = {"palette":{},"icons":{}};\n';

/** Text that is safe inside a <script> element. */
const safe = (s) => s.replace(/<\/(script)/gi, '<\\/$1').replace(/<!--/g, '<\\!--');
const script = (body) => `<script>\n${safe(body)}\n</script>`;

const built = [
  script(`window.PCT_TREE = ${JSON.stringify({ start, nodes, branches })};`),
  script(icons),
  ...SCRIPTS.map((file) => script(readFileSync(at('design/preview', file), 'utf8'))),
].join('\n');
const html = readFileSync(at('design/preview/template.html'), 'utf8').replace('<!--BUILT-->', () => built);
writeFileSync(at('design/preview.html'), html);

const drawn = Object.keys(JSON.parse(icons.slice(icons.indexOf('{'), icons.lastIndexOf('}') + 1)).icons ?? {}).length;
const ranked = nodes.filter((n) => n.ranks > 1).length;
console.log(
  `design/preview.html: ${nodes.length} nodes from ${treeFile.slice(ROOT.length + 1)}, ` +
    `${ranked} ranked, ${drawn} icons drawn, ${Math.round(html.length / 1024)} KB`,
);
