// Writes mixins/path_of_craft_tree/data/tree.json: all sixteen branches. The miner's nodes are
// defined here with their working effects; the others are read from the design spec's tables.
// Run with `cargo run --example gen_tree`. The nodes are laid out by examples/preview: runs of
// identical upgrades merged into ranked nodes, a layout without crossings, pixel-icon ids and
// branch colours. With `-- --preview` the same tree also goes to design/out for the HTML preview.
use serde_json::{Map, Value, json};

#[path = "preview/preview.rs"]
mod preview;

const OUTPUT: &str = "mixins/path_of_craft_tree/data/tree.json";
const PREVIEW_DIR: &str = "design/out";
const SPEC: &str = "docs/design.md";
const START_RADIUS: f32 = 500.0;
const SPINE_STEP: f32 = 34.0;
const SPINE_NODES: usize = 40;
const ZIGZAG: f32 = 10.0;
const BIG_OFFSET: f32 = 90.0;
const UPGRADE_STEP: f32 = 44.0;
const MAX_UPGRADES: usize = 3;
const RING_LINKS: [usize; 2] = [12, 28];
const GATHERING_BRANCHES: usize = 4;
const PICKUP_PER_BRIDGE: f64 = 0.5;
const RAY_ANGLE_DEGREES: f32 = 22.5;

type Effects = &'static [(&'static str, f64)];
type Upgrade = (&'static str, &'static str, Effects, bool);

struct Big {
    name: String,
    icon: String,
    description: String,
    effects: Value,
    todo: bool,
    upgrades: Vec<(String, String, Value, bool)>,
}

struct MinerBig {
    name: &'static str,
    icon: &'static str,
    description: &'static str,
    effects: Effects,
    todo: bool,
    upgrades: &'static [Upgrade],
}

const MINER_BIGS: [MinerBig; 10] = [
    MinerBig {
        name: "Жила",
        icon: "minecraft:iron_pickaxe",
        description: "Руда ломается цепочкой до 8 блоков, прочность тратится за каждый",
        effects: &[("vein_len", 8.0)],
        todo: false,
        upgrades: &[
            (
                "Жила: экономия",
                "Прочность за блок ×0,75",
                &[("vein_cost_cut", 0.25)],
                false,
            ),
            (
                "Жила: длина",
                "Цепочка до 12 блоков",
                &[("vein_len", 4.0)],
                false,
            ),
            (
                "Жила: экономия II",
                "Прочность за блок ×0,5",
                &[("vein_cost_cut", 0.25)],
                false,
            ),
            (
                "Жила: длина II",
                "Цепочка до 16 блоков",
                &[("vein_len", 4.0)],
                false,
            ),
            (
                "Жила: экономия III",
                "Прочность за блок ×0,25",
                &[("vein_cost_cut", 0.25)],
                false,
            ),
        ],
    },
    MinerBig {
        name: "Переплавка",
        icon: "minecraft:furnace",
        description: "Железо, медь и золото падают сразу слитками",
        effects: &[("smelt", 1.0)],
        todo: false,
        upgrades: &[
            (
                "Переплавка: шанс",
                "Шанс лишнего слитка 5%",
                &[("smelt_extra", 0.05)],
                false,
            ),
            (
                "Переплавка: шанс II",
                "Шанс лишнего слитка 10%",
                &[("smelt_extra", 0.05)],
                false,
            ),
        ],
    },
    MinerBig {
        name: "Прилив сил",
        icon: "minecraft:sugar",
        description: "После добычи руды 8 секунд ускоренной добычи",
        effects: &[("rush_seconds", 8.0)],
        todo: false,
        upgrades: &[
            (
                "Прилив: длительность",
                "12 секунд",
                &[("rush_seconds", 4.0)],
                false,
            ),
            ("Прилив: сила", "Спешка II", &[("rush_amp", 1.0)], false),
            (
                "Прилив: бег",
                "Дополнительно скорость передвижения",
                &[("rush_speed", 1.0)],
                false,
            ),
        ],
    },
    MinerBig {
        name: "Жадность",
        icon: "minecraft:diamond",
        description: "10% шанс лишнего алмаза или изумруда",
        effects: &[("greed", 0.10)],
        todo: false,
        upgrades: &[
            ("Жадность: 15%", "Шанс 15%", &[("greed", 0.05)], false),
            ("Жадность: 20%", "Шанс 20%", &[("greed", 0.05)], false),
        ],
    },
    MinerBig {
        name: "Забой",
        icon: "minecraft:tnt",
        description: "Переключаемый режим копания 3×3, прочность ×2",
        effects: &[],
        todo: true,
        upgrades: &[
            ("Забой: прочность", "Прочность ×1,5", &[], true),
            ("Забой: прочность II", "Прочность ×1,2", &[], true),
        ],
    },
    MinerBig {
        name: "Рудный нюх",
        icon: "minecraft:spyglass",
        description: "Подсветка руды в радиусе 6 блоков на 10 секунд",
        effects: &[],
        todo: true,
        upgrades: &[
            ("Нюх: радиус", "Радиус 8", &[], true),
            ("Нюх: радиус II", "Радиус 10", &[], true),
        ],
    },
    MinerBig {
        name: "Магнитная кирка",
        icon: "minecraft:iron_ingot",
        description: "Добыча сразу попадает в инвентарь",
        effects: &[],
        todo: true,
        upgrades: &[(
            "Магнит: опыт",
            "Опыт Minecraft тоже зачисляется сразу",
            &[],
            true,
        )],
    },
    MinerBig {
        name: "Твёрдая рука",
        icon: "minecraft:obsidian",
        description: "Обсидиан и обломки добываются вдвое быстрее",
        effects: &[],
        todo: true,
        upgrades: &[
            ("Рука: втрое", "Втрое быстрее", &[], true),
            ("Рука: сланец", "Действует и на глубинный сланец", &[], true),
        ],
    },
    MinerBig {
        name: "Глаза крота",
        icon: "minecraft:ender_eye",
        description: "Ночное зрение ниже уровня Y=0",
        effects: &[],
        todo: true,
        upgrades: &[
            ("Крот: порог", "Порог Y=40", &[], true),
            ("Крот: вода", "Действует под водой", &[], true),
        ],
    },
    MinerBig {
        name: "Закалка шахтёра",
        icon: "minecraft:iron_chestplate",
        description: "Меньше урона от падения и лавы под землёй, 25%",
        effects: &[],
        todo: true,
        upgrades: &[
            ("Закалка: 40%", "Снижение 40%", &[], true),
            ("Закалка: 50%", "Снижение 50%", &[], true),
        ],
    },
];

/// A small node: name, icon, effect key, value, whether the effect is not implemented yet.
type Small = (&'static str, &'static str, &'static str, f64, bool);

struct BranchMeta {
    id: &'static str,
    name: &'static str,
    tool_suffix: Option<&'static str>,
    smalls: [Small; 4],
    big_icons: [&'static str; 5],
}

const BRANCHES: [BranchMeta; 16] = [
    BranchMeta {
        id: "mining",
        name: "Шахтёр",
        tool_suffix: Some("_pickaxe"),
        big_icons: ["minecraft:iron_pickaxe"; 5],
        smalls: [
            (
                "Скорость кирки",
                "minecraft:stone_pickaxe",
                "speed:_pickaxe",
                0.015,
                false,
            ),
            (
                "Бережная работа",
                "minecraft:anvil",
                "durability:_pickaxe",
                0.02,
                false,
            ),
            (
                "Богатая порода",
                "minecraft:raw_iron",
                "extra_ore",
                0.012,
                false,
            ),
            (
                "Опыт с руды",
                "minecraft:experience_bottle",
                "mc_xp",
                0.03,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "digging",
        name: "Копатель",
        tool_suffix: Some("_shovel"),
        big_icons: [
            "minecraft:iron_shovel",
            "minecraft:gravel",
            "minecraft:brush",
            "minecraft:sand",
            "minecraft:clay_ball",
        ],
        smalls: [
            (
                "Скорость лопаты",
                "minecraft:stone_shovel",
                "speed:_shovel",
                0.015,
                false,
            ),
            (
                "Бережная работа",
                "minecraft:anvil",
                "durability:_shovel",
                0.02,
                false,
            ),
            (
                "Рыхлый грунт",
                "minecraft:gravel",
                "extra_digging",
                0.015,
                true,
            ),
            (
                "Находки",
                "minecraft:gold_nugget",
                "treasure_digging",
                0.005,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "woodcutting",
        name: "Лесоруб",
        tool_suffix: Some("_axe"),
        big_icons: [
            "minecraft:iron_axe",
            "minecraft:oak_sapling",
            "minecraft:oak_leaves",
            "minecraft:oak_planks",
            "minecraft:charcoal",
        ],
        smalls: [
            (
                "Скорость топора",
                "minecraft:stone_axe",
                "speed:_axe",
                0.015,
                false,
            ),
            (
                "Бережная работа",
                "minecraft:anvil",
                "durability:_axe",
                0.02,
                false,
            ),
            (
                "Толстый ствол",
                "minecraft:oak_log",
                "extra_log",
                0.012,
                true,
            ),
            (
                "Щедрая листва",
                "minecraft:oak_sapling",
                "extra_sapling",
                0.025,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "fishing",
        name: "Рыбак",
        tool_suffix: None,
        big_icons: [
            "minecraft:fishing_rod",
            "minecraft:cod",
            "minecraft:nautilus_shell",
            "minecraft:prismarine_shard",
            "minecraft:heart_of_the_sea",
        ],
        smalls: [
            ("Быстрый клёв", "minecraft:fishing_rod", "lure", 0.01, true),
            (
                "Удача рыбака",
                "minecraft:rabbit_foot",
                "sea_luck",
                0.01,
                true,
            ),
            (
                "Крепкая леска",
                "minecraft:string",
                "durability:_fishing_rod",
                0.02,
                false,
            ),
            (
                "Прибыльный улов",
                "minecraft:cod",
                "extra_fish",
                0.012,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "farming",
        name: "Фермер",
        tool_suffix: None,
        big_icons: [
            "minecraft:iron_hoe",
            "minecraft:wheat",
            "minecraft:carrot",
            "minecraft:melon_slice",
            "minecraft:sugar_cane",
        ],
        smalls: [
            ("Щедрая жатва", "minecraft:wheat", "extra_crop", 0.015, true),
            (
                "Скорость мотыги",
                "minecraft:stone_hoe",
                "speed:_hoe",
                0.015,
                false,
            ),
            (
                "Бережная работа",
                "minecraft:anvil",
                "durability:_hoe",
                0.02,
                false,
            ),
            (
                "Богатая земля",
                "minecraft:wheat_seeds",
                "extra_seeds",
                0.02,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "husbandry",
        name: "Скотовод",
        tool_suffix: None,
        big_icons: [
            "minecraft:wheat",
            "minecraft:lead",
            "minecraft:shears",
            "minecraft:bone",
            "minecraft:honey_bottle",
        ],
        smalls: [
            ("Забота", "minecraft:wheat", "breed_chance", 0.01, true),
            ("Ловкие руки", "minecraft:shears", "extra_wool", 0.015, true),
            ("Дрессировка", "minecraft:bone", "tame_chance", 0.02, true),
            (
                "Здоровое стадо",
                "minecraft:golden_carrot",
                "animal_health",
                0.02,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "cooking",
        name: "Повар",
        tool_suffix: None,
        big_icons: [
            "minecraft:cooked_beef",
            "minecraft:bread",
            "minecraft:cake",
            "minecraft:rabbit_stew",
            "minecraft:smoker",
        ],
        smalls: [
            ("Сытная еда", "minecraft:bread", "food_value", 0.015, true),
            (
                "Умелая готовка",
                "minecraft:furnace",
                "extra_food",
                0.012,
                true,
            ),
            (
                "Быстрый повар",
                "minecraft:smoker",
                "cook_speed",
                0.015,
                true,
            ),
            (
                "Ароматные блюда",
                "minecraft:cooked_beef",
                "food_regen",
                0.01,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "alchemy",
        name: "Алхимик",
        tool_suffix: None,
        big_icons: [
            "minecraft:brewing_stand",
            "minecraft:potion",
            "minecraft:nether_wart",
            "minecraft:blaze_powder",
            "minecraft:glass_bottle",
        ],
        smalls: [
            (
                "Долгие зелья",
                "minecraft:potion",
                "potion_duration",
                0.015,
                true,
            ),
            (
                "Быстрая варка",
                "minecraft:brewing_stand",
                "brew_speed",
                0.015,
                true,
            ),
            (
                "Экономный варщик",
                "minecraft:nether_wart",
                "brew_save",
                0.012,
                true,
            ),
            (
                "Стойкий организм",
                "minecraft:milk_bucket",
                "potion_resist",
                0.01,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "building",
        name: "Строитель",
        tool_suffix: None,
        big_icons: [
            "minecraft:bricks",
            "minecraft:scaffolding",
            "minecraft:stone_stairs",
            "minecraft:oak_planks",
            "minecraft:glass",
        ],
        smalls: [
            ("Длинные руки", "minecraft:stick", "reach", 0.02, true),
            (
                "Экономия блоков",
                "minecraft:bricks",
                "block_save",
                0.012,
                true,
            ),
            (
                "Твёрдая рука",
                "minecraft:stone",
                "build_speed",
                0.015,
                true,
            ),
            (
                "Лёгкий разбор",
                "minecraft:iron_pickaxe",
                "own_break_speed",
                0.02,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "crafting",
        name: "Крафтер",
        tool_suffix: None,
        big_icons: [
            "minecraft:crafting_table",
            "minecraft:redstone",
            "minecraft:rail",
            "minecraft:loom",
            "minecraft:cartography_table",
        ],
        smalls: [
            (
                "Запасливость",
                "minecraft:chest",
                "ingredient_save",
                0.012,
                true,
            ),
            (
                "Двойной результат",
                "minecraft:crafting_table",
                "craft_double",
                0.01,
                true,
            ),
            (
                "Экономия механизмов",
                "minecraft:redstone",
                "redstone_save",
                0.012,
                true,
            ),
            (
                "Опыт ремесла",
                "minecraft:experience_bottle",
                "craft_xp",
                0.02,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "smithing",
        name: "Кузнец",
        tool_suffix: None,
        big_icons: [
            "minecraft:anvil",
            "minecraft:iron_ingot",
            "minecraft:blast_furnace",
            "minecraft:smithing_table",
            "minecraft:netherite_ingot",
        ],
        smalls: [
            (
                "Прочность изделий",
                "minecraft:iron_ingot",
                "item_durability",
                0.015,
                true,
            ),
            (
                "Экономия топлива",
                "minecraft:coal",
                "fuel_save",
                0.015,
                true,
            ),
            (
                "Лишний слиток",
                "minecraft:iron_ingot",
                "extra_ingot",
                0.012,
                true,
            ),
            (
                "Дешёвая починка",
                "minecraft:anvil",
                "repair_save",
                0.015,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "enchanting",
        name: "Чародей",
        tool_suffix: None,
        big_icons: [
            "minecraft:enchanting_table",
            "minecraft:book",
            "minecraft:lapis_lazuli",
            "minecraft:experience_bottle",
            "minecraft:enchanted_book",
        ],
        smalls: [
            (
                "Дешёвые чары",
                "minecraft:lapis_lazuli",
                "enchant_save",
                0.012,
                true,
            ),
            (
                "Сильные чары",
                "minecraft:enchanted_book",
                "enchant_power",
                0.01,
                true,
            ),
            (
                "Поток опыта",
                "minecraft:experience_bottle",
                "xp_gain",
                0.015,
                true,
            ),
            ("Удачные чары", "minecraft:book", "enchant_luck", 0.01, true),
        ],
    },
    BranchMeta {
        id: "melee",
        name: "Воин",
        tool_suffix: None,
        big_icons: [
            "minecraft:iron_sword",
            "minecraft:creeper_head",
            "minecraft:iron_axe",
            "minecraft:redstone",
            "minecraft:gunpowder",
        ],
        smalls: [
            (
                "Сила удара",
                "minecraft:iron_sword",
                "melee_damage",
                0.01,
                true,
            ),
            (
                "Скорость атаки",
                "minecraft:feather",
                "attack_speed",
                0.01,
                true,
            ),
            (
                "Живучесть в бою",
                "minecraft:golden_apple",
                "lifesteal",
                0.005,
                true,
            ),
            (
                "Точный удар",
                "minecraft:spectral_arrow",
                "crit",
                0.01,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "archery",
        name: "Стрелок",
        tool_suffix: None,
        big_icons: [
            "minecraft:bow",
            "minecraft:crossbow",
            "minecraft:arrow",
            "minecraft:spectral_arrow",
            "minecraft:target",
        ],
        smalls: [
            (
                "Урон снарядов",
                "minecraft:arrow",
                "ranged_damage",
                0.01,
                true,
            ),
            ("Быстрая натяжка", "minecraft:bow", "draw_speed", 0.01, true),
            (
                "Экономия стрел",
                "minecraft:arrow",
                "arrow_save",
                0.015,
                true,
            ),
            (
                "Дальность",
                "minecraft:spectral_arrow",
                "projectile_speed",
                0.01,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "defense",
        name: "Страж",
        tool_suffix: None,
        big_icons: [
            "minecraft:shield",
            "minecraft:iron_chestplate",
            "minecraft:totem_of_undying",
            "minecraft:netherite_chestplate",
            "minecraft:golden_apple",
        ],
        smalls: [
            ("Броня", "minecraft:iron_chestplate", "armor", 0.01, true),
            (
                "Запас здоровья",
                "minecraft:golden_apple",
                "max_health",
                0.01,
                true,
            ),
            (
                "Крепкий щит",
                "minecraft:shield",
                "durability:shield",
                0.02,
                false,
            ),
            (
                "Стойкость",
                "minecraft:iron_boots",
                "knockback_resist",
                0.01,
                true,
            ),
        ],
    },
    BranchMeta {
        id: "exploration",
        name: "Исследователь",
        tool_suffix: None,
        big_icons: [
            "minecraft:compass",
            "minecraft:map",
            "minecraft:leather_boots",
            "minecraft:oak_boat",
            "minecraft:clock",
        ],
        smalls: [
            (
                "Скорость",
                "minecraft:leather_boots",
                "move_speed",
                0.008,
                true,
            ),
            (
                "Выносливость",
                "minecraft:bread",
                "hunger_save",
                0.015,
                true,
            ),
            (
                "Удача странника",
                "minecraft:compass",
                "loot_luck",
                0.01,
                true,
            ),
            ("Лёгкие шаги", "minecraft:feather", "fall_save", 0.015, true),
        ],
    },
];

fn effects_json(effects: &[(&str, f64)]) -> Value {
    Value::Object(
        effects
            .iter()
            .map(|(key, value)| ((*key).to_owned(), json!(value)))
            .collect(),
    )
}

fn miner_bigs() -> Vec<Big> {
    MINER_BIGS
        .iter()
        .map(|big| Big {
            name: big.name.to_owned(),
            icon: big.icon.to_owned(),
            description: big.description.to_owned(),
            effects: effects_json(big.effects),
            todo: big.todo,
            upgrades: big
                .upgrades
                .iter()
                .map(|(name, text, effects, todo)| {
                    (
                        (*name).to_owned(),
                        (*text).to_owned(),
                        effects_json(effects),
                        *todo,
                    )
                })
                .collect(),
        })
        .collect()
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

/// Turns "25%, затем 35%; режим 5×5" into one description per upgrade node.
fn split_upgrades(text: &str) -> Vec<String> {
    let parts: Vec<String> = text
        .split(';')
        .flat_map(|part| part.split(", затем "))
        .map(|part| part.trim().trim_end_matches('.').to_owned())
        .filter(|part| !part.is_empty())
        .collect();
    let single_word = parts.len() == 1 && !parts[0].contains(' ');
    if single_word {
        return vec![parts[0].clone(), format!("{} ещё", parts[0])];
    }
    parts.into_iter().take(MAX_UPGRADES).collect()
}

/// The ten big nodes of each branch, from the spec's tables, by branch id.
fn parse_spec(spec: &str, meta: &BranchMeta) -> Vec<Big> {
    let heading = format!("(`{}`)", meta.id);
    let mut lines = spec
        .lines()
        .skip_while(|line| !(line.starts_with("**") && line.contains(&heading)));
    let rows = lines
        .by_ref()
        .skip_while(|line| !line.starts_with("| Большой узел"))
        .skip(2)
        .take_while(|line| line.starts_with('|'));
    rows.map(table_cells)
        .zip(meta.big_icons.iter().cycle())
        .map(|(cells, icon)| {
            let name = cells[0].clone();
            let upgrades = split_upgrades(&cells[2])
                .into_iter()
                .enumerate()
                .map(|(index, text)| (format!("{name}: {}", index + 1), text, json!({}), true))
                .collect();
            Big {
                name,
                icon: (*icon).to_owned(),
                description: cells[1].clone(),
                effects: json!({}),
                todo: true,
                upgrades,
            }
        })
        .collect()
}

struct Ray {
    dir: [f32; 2],
    perp: [f32; 2],
}

impl Ray {
    fn new(index: usize) -> Ray {
        let angle = (-90.0 + RAY_ANGLE_DEGREES * index as f32).to_radians();
        let (sin, cos) = angle.sin_cos();
        Ray {
            dir: [cos, sin],
            perp: [-sin, cos],
        }
    }

    fn at(&self, radius: f32, side: f32) -> (f32, f32) {
        (
            self.dir[0] * radius + self.perp[0] * side,
            self.dir[1] * radius + self.perp[1] * side,
        )
    }
}

struct Builder {
    nodes: Vec<Value>,
    next: u16,
}

impl Builder {
    fn add(&mut self, branch: &str, node: Value) -> u16 {
        let id = self.next;
        self.next += 1;
        let mut node = node;
        node["id"] = json!(id);
        node["branch"] = json!(branch);
        self.nodes.push(node);
        id
    }

    fn add_big(
        &mut self,
        branch: &str,
        big: &Big,
        spine_id: u16,
        ray: &Ray,
        radius: f32,
        side: f32,
    ) {
        let (x, y) = ray.at(radius, side);
        let id = self.add(
            branch,
            json!({
                "x": x, "y": y, "icon": big.icon, "name": big.name, "kind": "big",
                "description": big.description, "effects": big.effects,
                "todo": big.todo, "links": [spine_id],
            }),
        );
        let mut previous = id;
        for (index, (name, description, effects, todo)) in big.upgrades.iter().enumerate() {
            let (x, y) = ray.at(radius + UPGRADE_STEP * (index as f32 + 1.0), side);
            previous = self.add(
                branch,
                json!({
                    "x": x, "y": y, "icon": big.icon, "name": name, "kind": "upgrade",
                    "description": description, "effects": effects,
                    "todo": todo, "links": [previous],
                }),
            );
        }
    }

    /// Adds one branch and returns the ids of its spine, from the centre outwards.
    fn add_branch(&mut self, meta: &BranchMeta, bigs: &[Big], ray: &Ray) -> Vec<u16> {
        let mut spine = Vec::with_capacity(SPINE_NODES);
        let mut previous = 0;
        let mut bigs = bigs.iter();
        for index in 0..SPINE_NODES {
            let (name, icon, key, value, todo) = meta.smalls[index % meta.smalls.len()];
            let todo = todo && !WIRED_SMALLS.contains(&key);
            let radius = START_RADIUS + SPINE_STEP * index as f32;
            let (x, y) = ray.at(radius, if index % 2 == 0 { -ZIGZAG } else { ZIGZAG });
            previous = self.add(
                meta.id,
                json!({
                    "x": x, "y": y, "icon": icon, "name": name, "kind": "small",
                    "effects": {key: value}, "todo": todo, "links": [previous],
                }),
            );
            spine.push(previous);
            if index % 4 == 3
                && let Some(big) = bigs.next() {
                    let side = if index % 8 == 3 {
                        -BIG_OFFSET
                    } else {
                        BIG_OFFSET
                    };
                    self.add_big(meta.id, big, previous, ray, radius, side);
                }
        }
        spine
    }

    /// A small node between two branches of the gathering sector: more pickup radius.
    fn add_bridge(&mut self, from: u16, to: u16) {
        let position = |id: u16| -> (f64, f64) {
            let node = self
                .nodes
                .iter()
                .find(|node| node["id"] == json!(id))
                .expect("bridge end");
            (
                node["x"].as_f64().expect("x"),
                node["y"].as_f64().expect("y"),
            )
        };
        let ((x1, y1), (x2, y2)) = (position(from), position(to));
        let id = self.next;
        self.next += 1;
        self.nodes.push(json!({
            "id": id, "x": (x1 + x2) / 2.0, "y": (y1 + y2) / 2.0, "icon": "minecraft:hopper",
            "name": "Притяжение", "kind": "small", "description": "Предметы притягиваются издалека",
            "effects": {"pickup_radius": PICKUP_PER_BRIDGE}, "links": [from, to],
        }));
    }

    fn link(&mut self, from: u16, to: u16) {
        if let Some(links) = self.nodes.iter_mut().find(|node| node["id"] == json!(from)) {
            links["links"]
                .as_array_mut()
                .expect("links")
                .push(json!(to));
        }
    }
}

fn group(blocks: &[&str], value: f64) -> Vec<(String, Value)> {
    blocks
        .iter()
        .map(|block| (format!("minecraft:blocks/{block}"), json!(value)))
        .collect()
}

fn action(key: &str, value: f64) -> (String, Value) {
    (format!("act:{key}"), json!(value))
}

fn branch_xp(id: &str) -> Map<String, Value> {
    let entries: Vec<(String, Value)> = match id {
        "mining" => [
            group(&["netherrack"], 0.25),
            group(
                &[
                    "stone",
                    "andesite",
                    "diorite",
                    "granite",
                    "tuff",
                    "basalt",
                    "blackstone",
                    "end_stone",
                ],
                0.5,
            ),
            group(&["deepslate"], 1.0),
            group(&["nether_gold_ore"], 3.0),
            group(
                &["copper_ore", "deepslate_copper_ore", "nether_quartz_ore"],
                4.0,
            ),
            group(&["coal_ore", "deepslate_coal_ore"], 5.0),
            group(
                &[
                    "iron_ore",
                    "deepslate_iron_ore",
                    "redstone_ore",
                    "deepslate_redstone_ore",
                ],
                8.0,
            ),
            group(&["lapis_ore", "deepslate_lapis_ore"], 12.0),
            group(&["obsidian", "gold_ore", "deepslate_gold_ore"], 15.0),
            group(&["diamond_ore", "deepslate_diamond_ore"], 50.0),
            group(&["emerald_ore", "deepslate_emerald_ore"], 60.0),
            group(&["ancient_debris"], 150.0),
        ]
        .concat(),
        "digging" => [
            group(&["snow"], 0.1),
            group(&["snow_block"], 0.2),
            group(
                &[
                    "dirt",
                    "grass_block",
                    "sand",
                    "red_sand",
                    "coarse_dirt",
                    "rooted_dirt",
                ],
                0.4,
            ),
            group(&["gravel", "mud"], 0.5),
            group(&["podzol", "mycelium", "soul_sand", "soul_soil"], 0.75),
            group(&["clay"], 1.0),
            vec![action("brush", 25.0), action("path", 0.4)],
        ]
        .concat(),
        "woodcutting" => [
            group(
                &[
                    "mushroom_stem",
                    "brown_mushroom_block",
                    "red_mushroom_block",
                ],
                0.5,
            ),
            group(
                &[
                    "oak_log",
                    "spruce_log",
                    "birch_log",
                    "jungle_log",
                    "acacia_log",
                    "dark_oak_log",
                    "mangrove_log",
                    "cherry_log",
                    "pale_oak_log",
                ],
                2.0,
            ),
            group(&["crimson_stem", "warped_stem"], 2.5),
            vec![action("strip", 0.25)],
        ]
        .concat(),
        "fishing" => vec![
            action("junk", 6.0),
            action("fish", 18.0),
            action("treasure", 50.0),
        ],
        "farming" => [
            group(&["bamboo"], 0.5),
            group(&["sugar_cane", "cactus", "sweet_berry_bush"], 1.0),
            group(&["melon", "pumpkin", "cocoa"], 2.0),
            group(
                &["wheat", "carrots", "potatoes", "beetroots", "nether_wart"],
                3.0,
            ),
            vec![action("bone_meal", 0.5)],
        ]
        .concat(),
        "husbandry" => vec![
            action("milk", 2.0),
            action("shear", 4.0),
            action("honey", 5.0),
            action("breed", 12.0),
            action("tame", 40.0),
        ],
        "cooking" => vec![
            action("eat", 1.0),
            action("cook", 4.0),
            action("campfire", 4.0),
            action("craft_food", 4.0),
            action("stew", 8.0),
            action("cake", 20.0),
        ],
        "alchemy" => vec![
            action("brew", 12.0),
            action("nether_wart", 3.0),
            action("brew_first", 25.0),
        ],
        "building" => vec![action("build", 1.5), action("deco", 2.0)],
        "crafting" => vec![action("craft", 0.5), action("craft_first", 15.0)],
        "smithing" => vec![
            action("ingot", 5.0),
            action("gear", 15.0),
            action("repair", 20.0),
            action("upgrade", 50.0),
        ],
        "enchanting" => vec![
            action("enchant", 60.0),
            action("book", 20.0),
            action("enchant_first", 50.0),
        ],
        "melee" => vec![action("kill", 1.0), action("boss", 3.0)],
        "archery" => vec![action("shot", 1.0)],
        "defense" => vec![action("hit", 4.0), action("block", 6.0)],
        "exploration" => vec![
            action("chunk", 1.0),
            action("trade", 10.0),
            action("advancement", 50.0),
            action("biome", 100.0),
            action("dimension", 500.0),
            action("structure", 200.0),
        ],
        other => panic!("no experience table for {other}"),
    };
    entries.into_iter().collect()
}

fn branches_json() -> Value {
    let map: Map<String, Value> = BRANCHES
        .iter()
        .map(|meta| {
            let mut branch = json!({"name": meta.name, "xp": branch_xp(meta.id)});
            if let Some(suffix) = meta.tool_suffix {
                branch["tool_suffix"] = json!(suffix);
            }
            (meta.id.to_owned(), branch)
        })
        .collect();
    Value::Object(map)
}

fn ores() -> Value {
    let table = |block: &str| format!("minecraft:blocks/{block}");
    let mut map = Map::new();
    let mut add = |blocks: &[&str], ore: Value| {
        for block in blocks {
            map.insert(table(block), ore.clone());
        }
    };
    let metal = |raw: &str, ingot: &str| json!({"raw": raw, "ingot": ingot, "drop": raw});
    let plain = |drop: &str| json!({"drop": drop});
    let rich = |drop: &str| json!({"drop": drop, "rich": true});
    add(
        &["iron_ore", "deepslate_iron_ore"],
        metal("minecraft:raw_iron", "minecraft:iron_ingot"),
    );
    add(
        &["copper_ore", "deepslate_copper_ore"],
        metal("minecraft:raw_copper", "minecraft:copper_ingot"),
    );
    add(
        &["gold_ore", "deepslate_gold_ore"],
        metal("minecraft:raw_gold", "minecraft:gold_ingot"),
    );
    add(&["coal_ore", "deepslate_coal_ore"], plain("minecraft:coal"));
    add(
        &["redstone_ore", "deepslate_redstone_ore"],
        plain("minecraft:redstone"),
    );
    add(
        &["lapis_ore", "deepslate_lapis_ore"],
        plain("minecraft:lapis_lazuli"),
    );
    add(&["nether_quartz_ore"], plain("minecraft:quartz"));
    add(&["nether_gold_ore"], plain("minecraft:gold_nugget"));
    add(
        &["diamond_ore", "deepslate_diamond_ore"],
        rich("minecraft:diamond"),
    );
    add(
        &["emerald_ore", "deepslate_emerald_ore"],
        rich("minecraft:emerald"),
    );
    add(&["ancient_debris"], plain("minecraft:ancient_debris"));
    Value::Object(map)
}

fn prefixed(items: &[&str]) -> Vec<String> {
    items
        .iter()
        .map(|item| format!("minecraft:{item}"))
        .collect()
}

fn lists() -> Value {
    json!({
        "fish": prefixed(&["cod", "salmon", "tropical_fish", "pufferfish"]),
        "treasure": prefixed(&["bow", "enchanted_book", "fishing_rod", "name_tag", "nautilus_shell", "saddle"]),
        "cooked_food": prefixed(&[
            "cooked_beef", "cooked_porkchop", "cooked_chicken", "cooked_mutton", "cooked_rabbit",
            "cooked_cod", "cooked_salmon", "baked_potato", "dried_kelp",
        ]),
        "raw_food": prefixed(&["beef", "porkchop", "chicken", "mutton", "rabbit", "cod", "salmon", "potato", "kelp"]),
        "crafted_food": prefixed(&["bread", "cookie", "pumpkin_pie"]),
        "stews": prefixed(&["mushroom_stew", "rabbit_stew", "beetroot_soup", "suspicious_stew"]),
        "ingots": prefixed(&["iron_ingot", "gold_ingot", "copper_ingot", "netherite_scrap"]),
        "gear_suffixes": [
            "_sword", "_pickaxe", "_axe", "_shovel", "_hoe", "_helmet", "_chestplate", "_leggings", "_boots",
        ],
        "reversible_recipes": prefixed(&[
            "iron_block", "iron_ingot_from_iron_block", "iron_nugget", "iron_ingot_from_nuggets",
            "gold_block", "gold_ingot_from_gold_block", "gold_nugget", "gold_ingot_from_nuggets",
            "diamond_block", "diamond", "emerald_block", "emerald", "lapis_block", "lapis_lazuli",
            "redstone_block", "redstone", "coal_block", "coal", "copper_block", "copper_ingot",
            "netherite_block", "netherite_ingot_from_netherite_block", "raw_iron_block", "raw_iron",
            "raw_gold_block", "raw_gold", "raw_copper_block", "raw_copper", "bone_block", "bone_meal_from_bone_block",
            "dried_kelp_block", "dried_kelp_from_dried_kelp_block", "hay_block", "wheat", "slime_block", "slime_ball",
        ]),
        "building_suffixes": [
            "stone", "_bricks", "bricks", "_planks", "glass", "_concrete", "_terracotta", "terracotta",
            "_wool", "cobblestone", "andesite", "diorite", "granite", "_tiles", "sandstone", "blackstone",
        ],
        "deco_suffixes": ["_stairs", "_slab", "_fence", "_wall", "_pane", "_fence_gate"],
        "not_building": prefixed(&["stone_button", "stone_pressure_plate"]),
        "bosses": prefixed(&["ender_dragon", "wither", "warden", "elder_guardian"]),
        "animals": prefixed(&[
            "cow", "pig", "sheep", "chicken", "rabbit", "horse", "donkey", "mule", "llama", "goat",
            "mooshroom", "fox", "bee", "camel", "sniffer", "turtle", "axolotl", "frog",
        ]),
        "ranged_damage": prefixed(&["arrow", "trident", "spectral_arrow"]),
        "hive_blocks": prefixed(&["beehive", "bee_nest"]),
        "strippable_suffixes": ["_log", "_stem"],
        "forest_biomes": prefixed(&[
            "forest", "birch_forest", "dark_forest", "flower_forest", "old_growth_birch_forest",
            "old_growth_pine_taiga", "old_growth_spruce_taiga", "taiga", "snowy_taiga", "jungle",
            "sparse_jungle", "bamboo_jungle", "windswept_forest", "grove", "cherry_grove",
            "mangrove_swamp", "pale_garden",
        ]),
        "nether_forest_biomes": prefixed(&["crimson_forest", "warped_forest"]),
        "tall_herbs": prefixed(&["tall_grass", "large_fern", "sunflower", "lilac", "rose_bush", "peony"]),
        "herbs": prefixed(&[
            "short_grass", "tall_grass", "fern", "large_fern", "dandelion", "poppy", "blue_orchid",
            "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip",
            "oxeye_daisy", "cornflower", "lily_of_the_valley", "sunflower", "lilac", "rose_bush",
            "peony", "torchflower", "pink_petals", "wildflowers",
        ]),
    })
}

/// The effect that grows with each branch's level: its main action.
fn level_bonus() -> Value {
    json!({
        "mining": "speed:_pickaxe", "digging": "speed:_shovel", "woodcutting": "speed:_axe",
        "fishing": "lure", "farming": "extra_crop", "husbandry": "breed_chance",
        "cooking": "food_value", "alchemy": "potion_duration", "building": "block_save",
        "crafting": "ingredient_save", "smithing": "item_durability", "enchanting": "enchant_save",
        "melee": "melee_damage", "archery": "ranged_damage", "defense": "armor",
        "exploration": "move_speed",
    })
}

fn numbers() -> Value {
    let table = |entries: &[(&str, f64)]| {
        Value::Object(
            entries
                .iter()
                .map(|(id, value)| {
                    // A bare id is vanilla's; another mod's keeps its own namespace.
                    let id = if id.contains(':') {
                        (*id).to_owned()
                    } else {
                        format!("minecraft:{id}")
                    };
                    (id, json!(value))
                })
                .collect(),
        )
    };
    json!({
        "ripe_age": table(&[("wheat", 7.0), ("carrots", 7.0), ("potatoes", 7.0), ("beetroots", 3.0), ("nether_wart", 3.0), ("cocoa", 2.0), ("sweet_berry_bush", 3.0)]),
        "mob_health": table(&[
            ("zombie", 20.0), ("husk", 20.0), ("drowned", 20.0), ("skeleton", 20.0), ("stray", 20.0), ("bogged", 16.0),
            ("creeper", 20.0), ("spider", 16.0), ("cave_spider", 12.0), ("enderman", 40.0), ("witch", 26.0),
            ("slime", 4.0), ("magma_cube", 4.0), ("phantom", 20.0), ("blaze", 20.0), ("ghast", 10.0),
            ("piglin", 16.0), ("piglin_brute", 50.0), ("hoglin", 40.0), ("zoglin", 40.0), ("zombified_piglin", 20.0),
            ("wither_skeleton", 20.0), ("guardian", 30.0), ("elder_guardian", 80.0), ("shulker", 30.0),
            ("vindicator", 24.0), ("pillager", 24.0), ("evoker", 24.0), ("ravager", 100.0), ("vex", 14.0),
            ("silverfish", 8.0), ("endermite", 8.0), ("breeze", 30.0), ("ender_dragon", 200.0), ("wither", 300.0),
            ("warden", 500.0), ("cow", 10.0), ("pig", 10.0), ("sheep", 8.0), ("chicken", 4.0), ("rabbit", 3.0),
            ("horse", 20.0), ("goat", 10.0), ("fox", 10.0), ("wolf", 8.0), ("cat", 10.0), ("llama", 20.0),
            // SoulsCraft's mobs, from its data/mobs.json stats.health (agreed 2026-11-13; SoulsCraft
            // sends new numbers when they change). The bog slime's size is folded into its pct.xp tag.
            ("soulscraft:revenant", 30.0), ("soulscraft:stalker", 22.0), ("soulscraft:marksman", 20.0),
            ("soulscraft:brute", 60.0), ("soulscraft:shrike", 14.0), ("soulscraft:lurker", 30.0),
            ("soulscraft:bog_slime", 16.0),
        ]),
    })
}

type Pairs = &'static [(&'static str, f64)];

/// Small-node effect keys the game already applies.
const WIRED_SMALLS: &[&str] = &[
    "extra_digging",
    "treasure_digging",
    "extra_log",
    "extra_sapling",
    "extra_crop",
    "armor",
    "animal_health",
    "arrow_save",
    "attack_speed",
    "block_save",
    "breed_chance",
    "brew_save",
    "brew_speed",
    "build_speed",
    "cook_speed",
    "craft_double",
    "craft_xp",
    "crit",
    "draw_speed",
    "enchant_luck",
    "enchant_power",
    "enchant_save",
    "extra_fish",
    "extra_food",
    "extra_ingot",
    "extra_seeds",
    "extra_wool",
    "fall_save",
    "food_regen",
    "food_value",
    "fuel_save",
    "hunger_save",
    "ingredient_save",
    "item_durability",
    "knockback_resist",
    "lifesteal",
    "loot_luck",
    "lure",
    "max_health",
    "mc_xp",
    "melee_damage",
    "move_speed",
    "own_break_speed",
    "pickup_radius",
    "potion_duration",
    "potion_resist",
    "projectile_speed",
    "ranged_damage",
    "reach",
    "redstone_save",
    "repair_save",
    "sea_luck",
    "tame_chance",
    "xp_gain",
];

/// Branch, big node, its effects, then the effects of each upgrade in order.
const WIRED_BIGS: &[(&str, &str, Pairs, &[Pairs])] = &[
    (
        "mining",
        "Глаза крота",
        &[("mole", 1.0)],
        &[&[("mole_depth", 40.0)], &[("mole_water", 1.0)]],
    ),
    (
        "mining",
        "Закалка шахтёра",
        &[("under_cut", 0.25)],
        &[&[("under_cut", 0.15)], &[("under_cut", 0.10)]],
    ),
    (
        "digging",
        "Кремнёвый глаз",
        &[("flint_always", 1.0)],
        &[&[("flint_double", 0.25)]],
    ),
    (
        "digging",
        "Кладоискатель",
        &[("treasure_digging", 0.01)],
        &[
            &[("treasure_digging", 0.01)],
            &[("treasure_digging", 0.01)],
            &[("treasure_rare", 1.0)],
        ],
    ),
    (
        "woodcutting",
        "Углежог",
        &[("charcoal", 0.10)],
        &[&[("charcoal", 0.10)], &[("charcoal", 0.10)]],
    ),
    (
        "woodcutting",
        "Лесной страж",
        &[("forest", 1.0)],
        &[&[("forest_amp", 1.0)], &[("forest_nether", 1.0)]],
    ),
    (
        "woodcutting",
        "Дары леса",
        &[("forest_gift", 0.005)],
        &[&[("forest_gift", 0.005)], &[("forest_gift_rich", 1.0)]],
    ),
    (
        "fishing",
        "Морская поступь",
        &[("sea_step", 1.0)],
        &[&[("sea_breath", 1.0)]],
    ),
    (
        "farming",
        "Золотой урожай",
        &[("golden_crop", 0.02)],
        &[&[("golden_crop", 0.02)], &[("golden_crop", 0.02)]],
    ),
    (
        "farming",
        "Дикие травы",
        &[("herbs_double", 1.0)],
        &[&[("herbs_seeds", 0.25)]],
    ),
    (
        "husbandry",
        "Мясник",
        &[("butcher", 0.5)],
        &[&[("butcher_hide", 0.25)]],
    ),
    (
        "melee",
        "Охотник за головами",
        &[("heads", 0.01)],
        &[&[("heads", 0.01)], &[("heads", 0.01)]],
    ),
    (
        "defense",
        "Каменная кожа",
        &[("stone_skin", 0.15)],
        &[&[("stone_skin", 0.15)], &[("stone_skin", 0.15)]],
    ),
    (
        "defense",
        "Второе дыхание",
        &[("second_wind", 1.0)],
        &[&[("second_wind_cut", 0.5)]],
    ),
    (
        "defense",
        "Огнестойкий",
        &[("fire_cut", 0.5)],
        &[&[("fire_cut", 0.25)], &[("fire_cut", 0.25)]],
    ),
    (
        "defense",
        "Восстановление",
        &[("idle_regen", 1.0)],
        &[&[("idle_regen", 1.0)], &[("idle_regen", 1.0)]],
    ),
    (
        "exploration",
        "Скороход",
        &[("swift", 1.0)],
        &[&[("swift", 1.0)], &[("swift", 1.0)]],
    ),
    (
        "exploration",
        "Мягкое приземление",
        &[("fall_cut", 0.5)],
        &[&[("fall_cut", 0.5)]],
    ),
    (
        "exploration",
        "Ночной путник",
        &[("night_walker", 1.0)],
        &[&[("night_walker_always", 1.0)]],
    ),
    (
        "mining",
        "Забой",
        &[("mine_area", 1.0), ("mine_area_cost", 2.0)],
        &[&[("mine_area_cost", -0.5)], &[("mine_area_cost", -0.3)]],
    ),
    (
        "mining",
        "Рудный нюх",
        &[("ore_sense", 6.0)],
        &[&[("ore_sense", 2.0)], &[("ore_sense", 2.0)]],
    ),
    (
        "mining",
        "Магнитная кирка",
        &[("magnet", 1.0)],
        &[&[("magnet_xp", 1.0)]],
    ),
    (
        "mining",
        "Твёрдая рука",
        &[("hard_hand", 1.0)],
        &[&[("hard_hand", 1.0)], &[("hard_hand_deepslate", 1.0)]],
    ),
    (
        "digging",
        "Экскаватор",
        &[("dig_area", 1.0), ("dig_area_cost", 2.0)],
        &[
            &[("dig_area_cost", -0.5)],
            &[("dig_area_cost", -0.3)],
            &[("dig_area_wide", 1.0)],
        ],
    ),
    (
        "digging",
        "Обрушение",
        &[("collapse", 8.0)],
        &[&[("collapse_cost_cut", 0.5)], &[("collapse", 8.0)]],
    ),
    (
        "digging",
        "Археолог",
        &[("brush_fast", 1.0)],
        &[&[("brush_extra", 0.10)], &[("brush_extra", 0.10)]],
    ),
    (
        "digging",
        "Стекольщик",
        &[("glass", 1.0)],
        &[&[("glass_extra", 0.10)], &[("glass_extra", 0.10)]],
    ),
    (
        "digging",
        "Засыпка",
        &[("fill", 1.0)],
        &[&[("fill_wide", 1.0)], &[("fill_save", 0.25)]],
    ),
    (
        "digging",
        "Ландшафтник",
        &[("path_area", 1.0)],
        &[&[("path_wide", 1.0)], &[("path_level", 1.0)]],
    ),
    (
        "digging",
        "Лёгкая поступь",
        &[("light_step", 1.0)],
        &[&[("light_step_snow", 1.0)]],
    ),
    (
        "digging",
        "Боевая лопата",
        &[("shovel_fight", 1.0)],
        &[&[("shovel_stun", 0.20)]],
    ),
    (
        "woodcutting",
        "Лесоповал",
        &[("timber", 32.0)],
        &[
            &[("timber_cost_cut", 0.75)],
            &[("timber", 32.0)],
            &[("timber_unlimited", 1.0)],
        ],
    ),
    (
        "woodcutting",
        "Лесничий",
        &[("forester", 1.0)],
        &[&[("forester_grow", 0.25)], &[("forester_2x2", 1.0)]],
    ),
    (
        "woodcutting",
        "Щедрая крона",
        &[("canopy", 1.0)],
        &[&[("canopy_bonus", 0.25)], &[("canopy_golden", 0.02)]],
    ),
    (
        "woodcutting",
        "Штабель",
        &[("stack_pile", 1.0)],
        &[&[("stack_inventory", 1.0)]],
    ),
    (
        "woodcutting",
        "Чистильщик",
        &[("strip_all", 1.0)],
        &[&[("strip_bonus", 0.25)]],
    ),
    (
        "woodcutting",
        "Плотник",
        &[("planks", 1.0)],
        &[&[("planks_extra", 1.0)], &[("planks_sticks", 0.25)]],
    ),
    (
        "woodcutting",
        "Живица",
        &[("resin", 8.0)],
        &[&[("resin", -3.0)], &[("resin", -2.0)]],
    ),
    (
        "fishing",
        "Морской клад",
        &[("sea_treasure", 1.0)],
        &[&[("sea_treasure", 1.0)], &[("sea_treasure", 1.0)]],
    ),
    (
        "fishing",
        "Быстрый клёв",
        &[("lure", 0.25)],
        &[&[("lure", 0.15)], &[("lure", 0.15)]],
    ),
    (
        "fishing",
        "Двойной улов",
        &[("double_catch", 0.15)],
        &[&[("double_catch", 0.10)], &[("double_catch", 0.10)]],
    ),
    (
        "fishing",
        "Чистая вода",
        &[("clean_water", 0.5)],
        &[&[("clean_water", 0.5)]],
    ),
    (
        "fishing",
        "Дар глубин",
        &[("deep_gift", 0.03)],
        &[&[("deep_gift_rich", 0.25)], &[("deep_gift_rich", 0.25)]],
    ),
    (
        "fishing",
        "Сырое не страшно",
        &[("raw_fish", 1.0)],
        &[&[("raw_fish_bonus", 1.0)]],
    ),
    (
        "fishing",
        "Гарпун",
        &[("harpoon", 1.0)],
        &[&[("harpoon_damage", 3.0)]],
    ),
    (
        "fishing",
        "Лавовая рыбалка",
        &[("lava_fish", 1.0)],
        &[&[("lava_loot", 1.0)]],
    ),
    (
        "fishing",
        "Рыбацкое счастье",
        &[("fish_luck", 0.15)],
        &[&[("fish_luck", 0.10)], &[("fish_luck", 0.10)]],
    ),
    (
        "farming",
        "Жнец",
        &[("reap", 1.0)],
        &[&[("reap_area", 1.0)], &[("reap_area", 1.0)]],
    ),
    (
        "farming",
        "Широкая мотыга",
        &[("till_radius", 1.0)],
        &[&[("till_radius", 1.0)], &[("till_radius", 1.0)]],
    ),
    (
        "farming",
        "Зелёные пальцы",
        &[("growth_radius", 4.0)],
        &[&[("growth_radius", 2.0)]],
    ),
    (
        "farming",
        "Вечная пашня",
        &[("no_trample", 1.0)],
        &[&[("wet_soil", 1.0)]],
    ),
    (
        "farming",
        "Семенной фонд",
        &[("seed_save", 0.15)],
        &[&[("seed_save", 0.10)], &[("seed_save", 0.10)]],
    ),
    (
        "farming",
        "Весь стебель",
        &[("whole_stalk", 1.0)],
        &[&[("stalk_inventory", 1.0)]],
    ),
    (
        "farming",
        "Пугало",
        &[("scarecrow", 8.0)],
        &[&[("scarecrow", 6.0)]],
    ),
    (
        "farming",
        "Сытная земля",
        &[("farm_food", 1.0)],
        &[&[("farm_food", 1.0)], &[("farm_food", 1.0)]],
    ),
    (
        "husbandry",
        "Заводчик",
        &[("twins", 0.15)],
        &[&[("twins", 0.10)], &[("twins", 0.10)]],
    ),
    (
        "husbandry",
        "Быстрое взросление",
        &[("grow_fast", 1.0)],
        &[&[("grow_fast", 1.0)], &[("grow_fast", 1.0)]],
    ),
    (
        "husbandry",
        "Любовь без пауз",
        &[("love_cut", 1.0)],
        &[&[("love_cut", 1.0)], &[("love_cut", 1.0)]],
    ),
    (
        "husbandry",
        "Приручитель",
        &[("tame_first", 0.25)],
        &[&[("tame_first", 0.25)], &[("tame_always", 1.0)]],
    ),
    (
        "husbandry",
        "Стригаль",
        &[("shear_extra", 0.5)],
        &[&[("shear_extra", 0.25)], &[("shear_extra", 0.25)]],
    ),
    (
        "husbandry",
        "Вожак",
        &[("lead_radius", 10.0)],
        &[&[("lead_radius", 6.0)], &[("lead_radius", 6.0)]],
    ),
    (
        "husbandry",
        "Верный пёс",
        &[("wolf_strong", 1.0)],
        &[&[("wolf_tough", 1.0)], &[("wolf_tough", 1.0)]],
    ),
    (
        "husbandry",
        "Конюх",
        &[("horse_boost", 1.0)],
        &[&[("horse_boost", 1.0)], &[("horse_boost", 1.0)]],
    ),
    (
        "husbandry",
        "Пчеловод",
        &[("bee_friend", 1.0), ("honey_extra", 0.5)],
        &[&[("honey_extra", 0.25)], &[("honey_extra", 0.25)]],
    ),
    (
        "cooking",
        "Гурман",
        &[("gourmet_seconds", 8.0)],
        &[&[("gourmet_seconds", 8.0)], &[("gourmet_seconds", 8.0)]],
    ),
    (
        "cooking",
        "Шеф",
        &[("chef", 0.10)],
        &[&[("chef", 0.10)], &[("chef", 0.10)]],
    ),
    (
        "cooking",
        "Быстрая плита",
        &[("cook_speed", 0.5)],
        &[&[("cook_speed", 0.25)], &[("cook_speed", 0.25)]],
    ),
    (
        "cooking",
        "Сытость",
        &[("saturate_mult", 0.5)],
        &[&[("saturate_mult", 0.25)], &[("saturate_mult", 0.25)]],
    ),
    (
        "cooking",
        "Едок на бегу",
        &[("eat_fast", 0.3)],
        &[&[("eat_run", 1.0)]],
    ),
    (
        "cooking",
        "Экономный едок",
        &[("food_save", 0.15)],
        &[&[("food_save", 0.10)], &[("food_save", 0.10)]],
    ),
    (
        "cooking",
        "Походная кухня",
        &[("soup_stack", 8.0)],
        &[&[("soup_stack", 8.0)]],
    ),
    (
        "cooking",
        "Дегустатор",
        &[("taster", 1.0)],
        &[&[("taster_poison", 1.0)]],
    ),
    (
        "cooking",
        "Угощение",
        &[("cake_share", 1.0)],
        &[&[("cake_amp", 1.0)], &[("cake_amp", 1.0)]],
    ),
    (
        "cooking",
        "Секретный рецепт",
        &[("secret_dishes", 1.0)],
        &[&[("secret_more", 1.0)]],
    ),
    (
        "alchemy",
        "Перегонка",
        &[("brew_save", 0.20)],
        &[&[("brew_save", 0.15)], &[("brew_save", 0.15)]],
    ),
    (
        "alchemy",
        "Продление",
        &[("potion_duration", 0.25)],
        &[&[("potion_duration", 0.25)], &[("potion_duration", 0.25)]],
    ),
    (
        "alchemy",
        "Концентрат",
        &[("potion_strong", 0.10)],
        &[&[("potion_strong", 0.10)], &[("potion_strong", 0.10)]],
    ),
    (
        "alchemy",
        "Быстрая варка",
        &[("brew_speed", 0.5)],
        &[&[("brew_speed", 0.25)], &[("brew_speed", 0.25)]],
    ),
    (
        "alchemy",
        "Щедрая стойка",
        &[("brew_extra", 0.10)],
        &[&[("brew_extra", 0.10)], &[("brew_extra", 0.10)]],
    ),
    (
        "alchemy",
        "Привычка к яду",
        &[("poison_weak", 1.0)],
        &[&[("poison_immune", 1.0)]],
    ),
    (
        "alchemy",
        "Широкий разлив",
        &[("splash_radius", 2.0)],
        &[&[("splash_radius", 2.0)]],
    ),
    (
        "alchemy",
        "Последний глоток",
        &[("potion_keep", 0.15)],
        &[&[("potion_keep", 0.10)], &[("potion_keep", 0.10)]],
    ),
    (
        "alchemy",
        "Опыт во флаконе",
        &[("brew_xp_bottle", 0.20)],
        &[&[("brew_xp_bottle", 0.15)], &[("brew_xp_bottle", 0.15)]],
    ),
    ("alchemy", "Смешение", &[("mix", 2.0)], &[&[("mix", 1.0)]]),
    (
        "building",
        "Экономный",
        &[("block_save", 0.10)],
        &[&[("block_save", 0.05)], &[("block_save", 0.05)]],
    ),
    (
        "building",
        "Длинные руки",
        &[("long_arms", 1.0)],
        &[&[("long_arms", 0.5)], &[("long_arms", 0.5)]],
    ),
    (
        "building",
        "Ровная линия",
        &[("line_len", 3.0)],
        &[&[("line_len", 2.0)], &[("line_len", 2.0)]],
    ),
    (
        "building",
        "Заливка",
        &[("fill_area", 64.0)],
        &[&[("fill_area", 192.0)]],
    ),
    (
        "building",
        "Замена",
        &[("replace", 1.0)],
        &[&[("replace_area", 1.0)]],
    ),
    (
        "building",
        "Быстрый разбор",
        &[("own_break_instant", 1.0)],
        &[&[("own_to_inv", 1.0)]],
    ),
    (
        "building",
        "Леса",
        &[("scaffold_speed", 1.0)],
        &[&[("scaffold_free", 1.0)]],
    ),
    (
        "building",
        "Без страха высоты",
        &[("no_fear", 1.0)],
        &[&[("no_fear_fall", 1.0)]],
    ),
    (
        "building",
        "Палитра",
        &[("palette", 1.0)],
        &[&[("palette_area", 1.0)]],
    ),
    (
        "building",
        "Чертёж",
        &[("blueprint", 5.0)],
        &[&[("blueprint", 4.0)]],
    ),
    (
        "crafting",
        "Мастерская",
        &[("craft_double", 0.10)],
        &[&[("craft_double", 0.05)], &[("craft_double", 0.05)]],
    ),
    (
        "crafting",
        "Запасливый",
        &[("ingredient_save", 0.10)],
        &[&[("ingredient_save", 0.05)], &[("ingredient_save", 0.05)]],
    ),
    (
        "crafting",
        "Карманный верстак",
        &[("pocket_table", 1.0)],
        &[&[("pocket_stations", 1.0)]],
    ),
    (
        "crafting",
        "Инженер",
        &[("redstone_save", 0.20)],
        &[&[("redstone_save", 0.10)], &[("redstone_save", 0.10)]],
    ),
    (
        "crafting",
        "Рельсовый мастер",
        &[("rail_bonus", 0.5)],
        &[&[("cart_tough", 0.5)]],
    ),
    (
        "crafting",
        "Каменотёс",
        &[("stonecutter_bonus", 0.25)],
        &[
            &[("stonecutter_bonus", 0.25)],
            &[("stonecutter_bonus", 0.25)],
        ],
    ),
    (
        "crafting",
        "Ткач",
        &[("weaver", 0.25)],
        &[&[("weaver", 0.25)], &[("weaver", 0.25)]],
    ),
    (
        "crafting",
        "Картограф",
        &[("map_wide", 1.0)],
        &[&[("map_save", 0.25)], &[("map_save", 0.25)]],
    ),
    (
        "crafting",
        "Массовый крафт",
        &[("mass_craft", 2.0)],
        &[&[("mass_craft", 1.0)], &[("mass_craft", 1.0)]],
    ),
    (
        "crafting",
        "Подручные средства",
        &[("nearby", 4.0)],
        &[&[("nearby", 4.0)]],
    ),
    (
        "smithing",
        "Закалка",
        &[("temper", 0.25)],
        &[&[("temper", 0.15)], &[("temper", 0.15)]],
    ),
    (
        "smithing",
        "Мастер наковальни",
        &[("anvil_master", 1.0)],
        &[&[("anvil_no_cap", 1.0)]],
    ),
    (
        "smithing",
        "Экономная плавка",
        &[("fuel_keep", 0.25)],
        &[&[("fuel_keep", 0.15)], &[("fuel_keep", 0.15)]],
    ),
    (
        "smithing",
        "Двойной слиток",
        &[("extra_ingot", 0.10)],
        &[&[("extra_ingot", 0.05)], &[("extra_ingot", 0.05)]],
    ),
    (
        "smithing",
        "Прочная ковка",
        &[("item_durability", 0.20)],
        &[&[("item_durability", 0.15)], &[("item_durability", 0.15)]],
    ),
    (
        "smithing",
        "Точильщик",
        &[("sharpen", 0.25)],
        &[&[("sharpen", 0.10)], &[("sharpen", 0.15)]],
    ),
    (
        "smithing",
        "Кольчужник",
        &[("chain_mail", 0.10)],
        &[&[("chain_mail", 0.05)], &[("chain_mail", 0.05)]],
    ),
    (
        "smithing",
        "Плавильщик",
        &[("smelt_speed", 0.25)],
        &[&[("smelt_speed", 0.20)], &[("smelt_speed", 0.25)]],
    ),
    (
        "smithing",
        "Разборка",
        &[("dismantle", 0.5)],
        &[&[("dismantle", 0.25)]],
    ),
    (
        "smithing",
        "Драгоценный сплав",
        &[("netherite_refund", 0.10)],
        &[&[("netherite_refund", 0.05)], &[("netherite_refund", 0.05)]],
    ),
    (
        "enchanting",
        "Резонанс",
        &[("resonance", 0.10)],
        &[&[("resonance", 0.10)], &[("resonance", 0.10)]],
    ),
    (
        "enchanting",
        "Экономия лазурита",
        &[("lapis_save", 0.5)],
        &[&[("lapis_save", 0.5)]],
    ),
    (
        "enchanting",
        "Обновление стола",
        &[("table_reroll", 1.0)],
        &[&[("table_reroll_free", 1.0)]],
    ),
    (
        "enchanting",
        "Знаток",
        &[("sage", 1.0)],
        &[&[("sage_odds", 1.0)]],
    ),
    (
        "enchanting",
        "Книжник",
        &[("shelves", 6.0)],
        &[&[("shelves_more", 1.0)]],
    ),
    (
        "enchanting",
        "Перенос",
        &[("transfer", 1.0)],
        &[&[("transfer_keep", 1.0)]],
    ),
    (
        "enchanting",
        "Точная точилка",
        &[("grind_book", 0.25)],
        &[&[("grind_book", 0.25)], &[("grind_book", 0.25)]],
    ),
    (
        "enchanting",
        "Поток опыта",
        &[("xp_flow", 0.20)],
        &[&[("xp_flow", 0.15)], &[("xp_flow", 0.15)]],
    ),
    (
        "enchanting",
        "Сверх предела",
        &[("beyond", 0.10)],
        &[&[("beyond", 0.10)], &[("beyond", 0.10)]],
    ),
    (
        "enchanting",
        "Запретная связь",
        &[("forbidden", 1.0)],
        &[&[("forbidden_more", 1.0)]],
    ),
    (
        "melee",
        "Кровопийца",
        &[("bloodsucker", 2.0)],
        &[&[("bloodsucker", 1.0)], &[("bloodsucker", 1.0)]],
    ),
    (
        "melee",
        "Шипы",
        &[("thorns_reflect", 0.2)],
        &[&[("thorns_reflect", 0.2)]],
    ),
    (
        "melee",
        "Добивание",
        &[("finisher", 0.25)],
        &[&[("finisher", 0.25)], &[("finisher", 0.25)]],
    ),
    (
        "melee",
        "Берсерк",
        &[("berserk", 0.25)],
        &[&[("berserk", 0.25)], &[("berserk", 0.25)]],
    ),
    (
        "melee",
        "Контратака",
        &[("counter", 0.4)],
        &[&[("counter", 0.3)], &[("counter", 0.3)]],
    ),
    (
        "melee",
        "Оглушение",
        &[("stun", 0.1)],
        &[&[("stun", 0.05)], &[("stun", 0.05)]],
    ),
    (
        "melee",
        "Кровотечение",
        &[("bleed", 1.0)],
        &[&[("bleed", 0.5)], &[("bleed", 0.5)]],
    ),
    (
        "melee",
        "Замедление детонации",
        &[("creeper_delay", 20.0)],
        &[
            &[("creeper_delay", 20.0)],
            &[("creeper_delay", 20.0), ("creeper_cancel", 1.0)],
        ],
    ),
    (
        "melee",
        "Боевой клич",
        &[("war_cry", 8.0)],
        &[&[("war_cry", 4.0)], &[("war_cry", 4.0)]],
    ),
    (
        "archery",
        "Меткий глаз",
        &[("arrow_save", 0.2)],
        &[&[("arrow_save", 0.15)], &[("arrow_save", 0.15)]],
    ),
    (
        "archery",
        "Скорострел",
        &[("draw_speed", 0.25)],
        &[&[("draw_speed", 0.25)], &[("draw_speed", 0.25)]],
    ),
    (
        "archery",
        "Дальнобой",
        &[("projectile_speed", 0.25)],
        &[&[("projectile_speed", 0.25)], &[("projectile_speed", 0.25)]],
    ),
    (
        "archery",
        "Тройной выстрел",
        &[("multi_shot", 2.0)],
        &[&[("multi_shot", 2.0)], &[("multi_shot", 2.0)]],
    ),
    (
        "archery",
        "Возврат стрел",
        &[("arrow_return", 0.4)],
        &[&[("arrow_return", 0.3)], &[("arrow_return", 0.3)]],
    ),
    (
        "archery",
        "Пробивная стрела",
        &[("pierce", 1.0)],
        &[&[("pierce", 2.0)]],
    ),
    (
        "archery",
        "Орлиный глаз",
        &[("eagle", 0.25)],
        &[&[("eagle", 0.15)], &[("eagle", 0.15)]],
    ),
    (
        "archery",
        "Охотничий след",
        &[("hunter_mark", 5.0)],
        &[&[("hunter_mark", 5.0)], &[("hunter_mark", 5.0)]],
    ),
    (
        "archery",
        "Быстрый арбалет",
        &[("quick_crossbow", 0.2)],
        &[&[("quick_crossbow", 0.2)], &[("quick_crossbow", 0.2)]],
    ),
    (
        "archery",
        "Стрельба на ходу",
        &[("shoot_move", 1.0)],
        &[&[("shoot_move_crossbow", 1.0)]],
    ),
    (
        "defense",
        "Несгибаемый",
        &[("unyielding", 0.5)],
        &[&[("unyielding", 0.5)]],
    ),
    (
        "defense",
        "Щит-стена",
        &[("shield_wall", 1.0)],
        &[&[("shield_wide", 1.0)]],
    ),
    (
        "defense",
        "Удар щитом",
        &[("shield_bash", 1.0)],
        &[&[("shield_bash_damage", 3.0)]],
    ),
    (
        "defense",
        "Опора",
        &[("ally_guard", 8.0)],
        &[&[("ally_guard", 4.0)], &[("ally_guard", 4.0)]],
    ),
    (
        "defense",
        "Провокация",
        &[("taunt", 8.0)],
        &[&[("taunt", 4.0)], &[("taunt", 4.0)]],
    ),
    (
        "defense",
        "Тяжёлая поступь",
        &[("heavy_step", 1.0)],
        &[&[("heavy_step", 1.0)], &[("heavy_step", 1.0)]],
    ),
    (
        "exploration",
        "Картограф",
        &[("cartographer", 1.0)],
        &[&[("cartographer_more", 1.0)]],
    ),
    (
        "exploration",
        "Неутомимый",
        &[("hunger_save", 0.25)],
        &[&[("hunger_save", 0.15)], &[("hunger_save", 0.15)]],
    ),
    (
        "exploration",
        "Горный шаг",
        &[("step_up", 0.4)],
        &[&[("step_up", 1.0)]],
    ),
    (
        "exploration",
        "Капитан",
        &[("captain", 0.25)],
        &[&[("captain", 0.25)], &[("captain", 0.25)]],
    ),
    (
        "exploration",
        "Возвращение домой",
        &[("home", 1.0)],
        &[&[("home_cut", 0.5)]],
    ),
    (
        "exploration",
        "Чутьё сокровищ",
        &[("treasure_sense", 1.0)],
        &[&[("treasure_more", 1.0)]],
    ),
    (
        "exploration",
        "Обаяние",
        &[("charm", 30.0)],
        &[&[("charm", 30.0)]],
    ),
];

/// Gives the nodes in `WIRED_BIGS` their working effects; a mismatch with the spec stops the build.
fn apply_wired(branch: &str, bigs: &mut [Big]) {
    for (_, name, effects, upgrades) in WIRED_BIGS.iter().filter(|row| row.0 == branch) {
        let big = bigs
            .iter_mut()
            .find(|big| big.name == *name)
            .unwrap_or_else(|| panic!("{branch}: no big node {name}"));
        assert_eq!(
            big.upgrades.len(),
            upgrades.len(),
            "{branch}: upgrades of {name}"
        );
        big.effects = effects_json(effects);
        big.todo = false;
        for (upgrade, wired) in big.upgrades.iter_mut().zip(upgrades.iter()) {
            upgrade.2 = effects_json(wired);
            upgrade.3 = false;
        }
    }
}

fn main() -> std::io::Result<()> {
    let spec = std::fs::read_to_string(SPEC)?;
    let start = json!({
        "id": 0, "x": 0, "y": 0, "icon": "minecraft:nether_star", "name": "Начало",
        "kind": "start", "description": "Отсюда растёт дерево",
    });
    let mut builder = Builder {
        nodes: vec![start],
        next: 1,
    };
    let mut spines = Vec::with_capacity(BRANCHES.len());
    for (index, meta) in BRANCHES.iter().enumerate() {
        let mut bigs = if meta.id == "mining" {
            miner_bigs()
        } else {
            parse_spec(&spec, meta)
        };
        apply_wired(meta.id, &mut bigs);
        assert_eq!(
            bigs.len(),
            10,
            "{} has {} big nodes in the spec",
            meta.id,
            bigs.len()
        );
        spines.push(builder.add_branch(meta, &bigs, &Ray::new(index)));
    }
    for index in 0..spines.len() {
        let next = (index + 1) % spines.len();
        for ring in RING_LINKS {
            let (from, to) = (spines[index][ring], spines[next][ring]);
            if index < GATHERING_BRANCHES - 1 {
                builder.add_bridge(from, to);
            } else {
                builder.link(from, to);
            }
        }
    }
    let tree = json!({
        "start": 0,
        "general_share": 0.15,
        "cross_share": 0.8,
        "machine_share": 0.5,
        "curve": {"coefficient": 9.0, "exponent": 1.5, "max_level": 75},
        "branches": branches_json(),
        "ores": ores(),
        "smelt_cross": {"branch": "smithing", "xp": 5.0},
        "base_attributes": {},
        "level_bonus": level_bonus(),
        "lists": lists(),
        "numbers": numbers(),
        "nodes": builder.nodes,
    });
    let tree = preview::convert(&tree);
    let text = serde_json::to_string(&tree)?;
    let mut outputs = vec![OUTPUT];
    if std::env::args().any(|arg| arg == "--preview") {
        std::fs::create_dir_all(PREVIEW_DIR)?;
        outputs.push(preview::OUTPUT);
    }
    for output in outputs {
        std::fs::write(output, &text)?;
        println!(
            "wrote {output}: {} nodes, {} bytes",
            tree["nodes"].as_array().map_or(0, Vec::len),
            text.len()
        );
    }
    Ok(())
}
