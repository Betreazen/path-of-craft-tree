// Lists every icon the tree needs, from the current tree data, into design/icon-list.json.
//   node design/tools/icon-list.mjs   (run from the repository root)
import { readFileSync, writeFileSync } from 'node:fs';

const tree = JSON.parse(readFileSync('mixins/path_of_craft_tree/data/tree.json', 'utf8'));
const byId = new Map(tree.nodes.map((n) => [n.id, n]));
const SECTOR = {
  mining: 'gather', digging: 'gather', woodcutting: 'gather', fishing: 'gather',
  farming: 'farm', husbandry: 'farm', cooking: 'farm', alchemy: 'farm',
  building: 'craft', crafting: 'craft', smithing: 'craft', enchanting: 'craft',
  melee: 'roam', archery: 'roam', defense: 'roam', exploration: 'roam',
};
const icons = new Map();
const add = (id, entry) => {
  if (!icons.has(id)) icons.set(id, { id, ...entry });
};

add('start', { kind: 'start', sector: 'gather', name: 'Начало', about: 'Центр дерева' });
for (const [id, branch] of Object.entries(tree.branches)) {
  add(`branch:${id}`, {
    kind: 'branch', sector: SECTOR[id], branch: id, name: branch.name,
    about: `Эмблема ветки «${branch.name}»`,
  });
}

// Upgrades inherit the icon of the big node they grow from.
const upgradesOf = new Map();
for (const n of tree.nodes.filter((n) => n.kind === 'upgrade')) {
  let parent = byId.get(n.links[0]);
  while (parent && parent.kind === 'upgrade') parent = byId.get(parent.links[0]);
  const list = upgradesOf.get(parent.name) ?? [];
  list.push(`${n.name}: ${n.description ?? ''}`);
  upgradesOf.set(parent.name, list);
}
for (const n of tree.nodes.filter((n) => n.kind === 'big')) {
  add(`big:${n.name}`, {
    kind: 'big', sector: SECTOR[n.branch], branch: n.branch, name: n.name,
    about: n.description ?? '', upgrades: upgradesOf.get(n.name) ?? [], old_icon: n.icon,
  });
}
for (const n of tree.nodes.filter((n) => n.kind === 'small')) {
  const key = Object.keys(n.effects)[0];
  add(`small:${key}`, {
    kind: 'small', sector: SECTOR[n.branch] ?? 'gather', branch: n.branch ?? null, name: n.name,
    about: n.description ?? `${key} +${n.effects[key]}`, old_icon: n.icon,
  });
}

const list = [...icons.values()];
writeFileSync('design/icon-list.json', JSON.stringify(list, null, 1));
const count = (s) => list.filter((i) => i.sector === s).length;
console.log(`icons: ${list.length}; gather ${count('gather')}, farm ${count('farm')}, craft ${count('craft')}, roam ${count('roam')}`);
