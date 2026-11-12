// Builds docs/showcase/js/data.js, the data of the mod's guide page, from the mod itself:
//   node design/tools/showcase-data.mjs   (works from any folder)
// Numbers come from mixins/path_of_craft_tree/data/tree.json and the Rust constants; the branch
// prose from docs/design.md section 2; the machine reports from docs/machine-reports.md; the icons
// from the game atlas data/atlas.png. Only the Russian names of blocks and actions live here.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (...parts) => readFileSync(join(ROOT, ...parts), 'utf8');
const MOD = 'mixins/path_of_craft_tree';

const tree = JSON.parse(read(MOD, 'data/tree.json'));
const atlas = JSON.parse(read(MOD, 'data/atlas.json'));
const atlasPng = readFileSync(join(ROOT, MOD, 'data/atlas.png')).toString('base64');
const design = read('docs/design.md');
const machines = read('docs/machine-reports.md');

// ---- Constants of the Rust code, read rather than copied ----
const rust = (file) => read(MOD, file);
const constant = (file, name) => {
  const m = rust(file).match(new RegExp(`const ${name}: [^=]+= ([^;]+);`));
  if (!m) throw new Error(`${name} not found in ${file}`);
  return m[1].trim();
};
const number = (file, name) => Number(constant(file, name).replace(/_/g, ''));
const seconds = (file, name) => {
  const v = constant(file, name).match(/from_secs\(([^)]+)\)/)[1];
  return v.split('*').reduce((p, f) => p * Number(f.trim()), 1);
};
const CONSTS = {
  levelBonus: number('rules.rs', 'LEVEL_BONUS'),
  respecLevels: number('rules.rs', 'RESPEC_LEVELS'),
  spawnerShare: number('fx_roam_rules.rs', 'SPAWNER_XP_SHARE'),
  farShotBonus: number('fx_roam_rules.rs', 'FAR_SHOT_BONUS'),
  milkCooldown: seconds('actions.rs', 'MILK_COOLDOWN'),
  rebuildCooldown: seconds('actions.rs', 'REBUILD_COOLDOWN'),
  hitXpPerSecond: number('actions.rs', 'MAX_HIT_XP_PER_WINDOW'),
  bottlesPerBrew: number('actions.rs', 'BOTTLES_PER_BREW'),
  generalShare: tree.general_share,
  crossShare: tree.cross_share,
  machineShare: tree.machine_share,
  curve: tree.curve,
  openKey: defaultKey('tree'),
};

// The default of one of the tree's rebindable keys in keys.rs, as a letter: `K`.
function defaultKey(name) {
  const m = rust('keys.rs').match(new RegExp(`"${name}",\\s*"[^"]*",\\s*"key\\.keyboard\\.([a-z0-9.]+)"`));
  if (!m) throw new Error(`key ${name} not found in keys.rs`);
  return m[1].toUpperCase();
}

// ---- Sectors and branches, in the order of docs/design.md ----
const SECTORS = [
  ['gather', 'Добыча', ['mining', 'digging', 'woodcutting', 'fishing']],
  ['farm', 'Хозяйство', ['farming', 'husbandry', 'cooking', 'alchemy']],
  ['craft', 'Ремесло', ['building', 'crafting', 'smithing', 'enchanting']],
  ['roam', 'Странствия', ['melee', 'archery', 'defense', 'exploration']],
];

const BLOCKS = {
  clay: 'Глина', coarse_dirt: 'Каменистая земля', dirt: 'Земля', grass_block: 'Блок травы', gravel: 'Гравий',
  mud: 'Ил', mycelium: 'Мицелий', podzol: 'Подзол', red_sand: 'Красный песок', rooted_dirt: 'Земля с корнями',
  sand: 'Песок', snow: 'Снежный слой', snow_block: 'Блок снега', soul_sand: 'Песок душ', soul_soil: 'Почва душ',
  bamboo: 'Бамбук', beetroots: 'Свёкла', cactus: 'Кактус', carrots: 'Морковь', cocoa: 'Какао-бобы', melon: 'Арбуз',
  nether_wart: 'Незерский нарост', potatoes: 'Картофель', pumpkin: 'Тыква', sugar_cane: 'Сахарный тростник',
  sweet_berry_bush: 'Куст сладких ягод', wheat: 'Пшеница', ancient_debris: 'Древние обломки', andesite: 'Андезит',
  basalt: 'Базальт', blackstone: 'Чернокамень', coal_ore: 'Угольная руда', copper_ore: 'Медная руда',
  deepslate: 'Глубинный сланец', diamond_ore: 'Алмазная руда', diorite: 'Диорит', emerald_ore: 'Изумрудная руда',
  end_stone: 'Эндерняк', gold_ore: 'Золотая руда', granite: 'Гранит', iron_ore: 'Железная руда',
  lapis_ore: 'Лазуритовая руда', nether_gold_ore: 'Незерская золотая руда', nether_quartz_ore: 'Кварцевая руда',
  netherrack: 'Незерак', obsidian: 'Обсидиан', redstone_ore: 'Редстоуновая руда', stone: 'Камень', tuff: 'Туф',
  acacia_log: 'Акация', birch_log: 'Берёза', brown_mushroom_block: 'Блок коричневого гриба', cherry_log: 'Вишня',
  crimson_stem: 'Багровый стебель', dark_oak_log: 'Тёмный дуб', jungle_log: 'Тропическое дерево',
  mangrove_log: 'Мангровое дерево', mushroom_stem: 'Ножка гриба', oak_log: 'Дуб', pale_oak_log: 'Бледный дуб',
  red_mushroom_block: 'Блок красного гриба', spruce_log: 'Ель', warped_stem: 'Искажённый стебель',
};

const ACTIONS = {
  'act:brew': 'Варка зелья, за бутылку',
  'act:brew_first': 'Первое зелье каждого вида',
  'act:nether_wart': 'Сбор незерского нароста',
  'act:shot': 'Убийство снарядом, за единицу здоровья моба',
  'act:kill': 'Убийство в ближнем бою, за единицу здоровья моба',
  'act:boss': 'Множитель за босса',
  'act:build': 'Строительный блок на новом месте',
  'act:deco': 'Декоративный блок на новом месте',
  'act:cake': 'Торт',
  'act:campfire': 'Еда, приготовленная на костре',
  'act:cook': 'Еда из печи',
  'act:craft_food': 'Крафт еды',
  'act:eat': 'Съеденная приготовленная еда',
  'act:stew': 'Рагу и супы',
  'act:craft': 'Крафт, за каждый раз',
  'act:craft_first': 'Первый крафт предмета',
  'act:block': 'Урон, принятый щитом, за единицу',
  'act:hit': 'Урон от мобов, за единицу',
  'act:brush': 'Находка кисточкой',
  'act:path': 'Тропинка лопатой, первый раз на блоке',
  'act:book': 'Книга на наковальне, за уровень',
  'act:enchant': 'Зачарование на столе, за уровень',
  'act:enchant_first': 'Первое зачарование каждого вида',
  'act:advancement': 'Достижение',
  'act:biome': 'Новый биом',
  'act:chunk': 'Новый чанк',
  'act:dimension': 'Новое измерение',
  'act:structure': 'Новая постройка',
  'act:trade': 'Сделка с жителем',
  'act:bone_meal': 'Костная мука на растущее растение',
  'act:fish': 'Рыба',
  'act:junk': 'Мусор',
  'act:treasure': 'Сокровище',
  'act:breed': 'Рождение детёныша',
  'act:honey': 'Мёд или соты из полного улья',
  'act:milk': 'Дойка',
  'act:shear': 'Стрижка',
  'act:tame': 'Приручение',
  'act:gear': 'Крафт инструмента или брони',
  'act:ingot': 'Слиток из печи',
  'act:repair': 'Ремонт на наковальне',
  'act:upgrade': 'Улучшение на столе кузнеца',
  'act:strip': 'Снятие коры с бревна',
};

const TOOLS = { _pickaxe: 'кирка', _shovel: 'лопата', _axe: 'топор' };

const fail = (msg) => {
  throw new Error(msg);
};

/** The branch's experience table: actions first, then blocks grouped by value. */
function xpTable(xp) {
  const rows = [];
  const blocks = new Map();
  for (const [key, value] of Object.entries(xp)) {
    if (key.startsWith('act:')) {
      rows.push([ACTIONS[key] ?? fail(`no name for ${key}`), value, key === 'act:boss' ? 'x' : '']);
      continue;
    }
    const id = key.replace('minecraft:blocks/', '');
    const base = id.replace(/^deepslate_(.+_ore)$/, '$1');
    if (base !== id && xp[`minecraft:blocks/${base}`] === value) continue;
    let name = BLOCKS[id] ?? fail(`no name for ${id}`);
    if (base === id && xp[`minecraft:blocks/deepslate_${id}`] === value) name += ' и глубинная';
    if (!blocks.has(value)) blocks.set(value, []);
    blocks.get(value).push(name);
  }
  const grouped = [...blocks.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([value, names]) => [names.map((n, i) => (i ? n.toLowerCase() : n)).join(', '), value, '']);
  return [...rows.sort((a, b) => a[1] - b[1]), ...grouped];
}

/** The prose about a branch in docs/design.md section 2, without its first-draft tables. */
function prose(name, id) {
  const lines = design.split('\n');
  const startAt = lines.findIndex((l) => l.startsWith(`**${name}** (\`${id}\`)`));
  if (startAt < 0) fail(`no design text for ${id}`);
  const out = [];
  for (let i = startAt; i < lines.length; i++) {
    const line = lines[i];
    if (i > startAt && (/^\*\*[^*]+\*\* \(`/.test(line) || line.startsWith('#'))) break;
    if (!line.trim() || line.startsWith('|')) continue;
    let text = line
      .replace(/^\*\*[^*]+\*\* \(`[a-z_]+`\)\.\s*/, '')
      .replace(/Ветвь обсуждена\.\s*/, '')
      .trim();
    // Level bonuses and the sums of small nodes are worked out from the data instead.
    text = text
      .split(/(?<=\.)\s+/)
      .filter((s) => !/^(Уровень ветви даёт|Малые узлы)/.test(s))
      .join(' ');
    if (text) out.push(text);
  }
  return out;
}

const byId = new Map(tree.nodes.map((n) => [n.id, n]));
/** The big node an upgrade improves, through any upgrades before it. */
const bigOf = (n) => {
  let p = byId.get(n.links?.[0]);
  const seen = new Set();
  while (p && p.kind === 'upgrade' && !seen.has(p.id)) {
    seen.add(p.id);
    p = byId.get(p.links?.[0]);
  }
  return p?.kind === 'big' ? p : null;
};

const pct = (v) => Math.round(v * 1000) / 10;

const branches = {};
for (const [sector, , ids] of SECTORS) {
  for (const id of ids) {
    const b = tree.branches[id] ?? fail(`no branch ${id}`);
    const own = tree.nodes.filter((n) => n.branch === id);
    const kinds = { small: 0, big: 0, upgrade: 0 };
    own.forEach((n) => kinds[n.kind]++);
    const sums = new Map();
    for (const n of own.filter((n) => n.kind === 'small')) {
      for (const [key, v] of Object.entries(n.effects ?? {})) {
        const k = `${n.name}\u0000${key}`;
        sums.set(k, (sums.get(k) ?? 0) + v * (n.ranks ?? 1));
      }
    }
    const bonusKey = tree.level_bonus[id];
    const bonusName = own.find((n) => n.kind === 'small' && n.effects?.[bonusKey] !== undefined)?.name;
    const bigs = own
      .filter((n) => n.kind === 'big')
      .map((big) => ({ id: big.id, ups: own.filter((u) => u.kind === 'upgrade' && bigOf(u) === big).map((u) => u.id) }));
    branches[id] = {
      name: b.name,
      colour: b.colour,
      sector,
      tool: TOOLS[b.tool_suffix] ?? '',
      kinds,
      ranked: own.filter((n) => (n.ranks ?? 1) > 1).length,
      xp: xpTable(b.xp),
      prose: prose(b.name, id),
      bonus: { name: bonusName ?? bonusKey, perLevel: pct(CONSTS.levelBonus) },
      sums: [...sums.entries()].map(([k, v]) => [k.split('\u0000')[0], pct(v)]).sort((a, b) => b[1] - a[1]),
      bigs,
    };
  }
}

// ---- Nodes, short keys to keep the file small ----
const nodes = tree.nodes.map((n) => {
  const o = { id: n.id, x: n.x, y: n.y, k: n.kind, i: n.icon, n: n.name };
  if (n.branch) o.b = n.branch;
  if (n.description) o.d = n.description;
  if (n.effects && Object.keys(n.effects).length) o.e = Object.values(n.effects);
  if ((n.ranks ?? 1) > 1) {
    o.r = n.ranks;
    o.rd = n.rank_descriptions ?? [];
  }
  if (n.links?.length) o.l = n.links;
  return o;
});

// ---- Levels: experience for the next level is floor(c × level^e) ----
const { coefficient, exponent, max_level: maxLevel } = tree.curve;
const levels = [];
let total = 0;
for (let level = 1; level <= maxLevel; level++) {
  const next = Math.floor(coefficient * level ** exponent);
  total += next;
  if ([1, 5, 10, 25, 50, maxLevel].includes(level)) levels.push([level, next, total]);
}

// ---- Machine reports: the table in docs/machine-reports.md ----
const reports = [];
const reportSection = machines.split(/^## /m).find((s) => s.startsWith('Отчёты'));
for (const line of (reportSection ?? '').split('\n')) {
  // A row may name several kinds that share one format: `repair`, `breed`, ...
  const m = line.match(/^\| (`[a-z-]+`(?:, `[a-z-]+`)*) \| `([^`]+)` \| (.+?) \|$/);
  if (m) reports.push([m[1].replace(/`/g, ''), m[2], m[3]]);
}
if (!reports.length) fail('no report table in docs/machine-reports.md');

// ---- Sprites the page draws ----
const sprites = Object.fromEntries(
  Object.entries(atlas.sprites).filter(([k]) => /^(c?frame|c?icon|banner|badge|pips|stone):/.test(k)),
);

const counts = { nodes: tree.nodes.length, ...Object.fromEntries(['small', 'big', 'upgrade'].map((k) => [k, tree.nodes.filter((n) => n.kind === k).length])) };
counts.ranked = tree.nodes.filter((n) => (n.ranks ?? 1) > 1).length;
counts.effects = new Set(tree.nodes.flatMap((n) => Object.keys(n.effects ?? {}))).size;

const data = {
  atlas: `data:image/png;base64,${atlasPng}`,
  sprites,
  start: tree.start,
  nodes,
  sectors: SECTORS,
  branches,
  consts: CONSTS,
  counts,
  levels,
  reports,
  smeltCross: tree.smelt_cross,
};

const out = join(ROOT, 'docs/showcase/js/data.js');
writeFileSync(out, `'use strict';\n// Made by design/tools/showcase-data.mjs from the mod's data; do not edit by hand.\nwindow.PCTS = ${JSON.stringify(data)};\n`);
console.log(`wrote docs/showcase/js/data.js: ${nodes.length} nodes, ${reports.length} reports, ${Object.keys(sprites).length} sprites`);
