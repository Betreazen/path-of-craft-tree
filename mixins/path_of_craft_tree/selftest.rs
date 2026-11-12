// Self-test for the joint bench of the pack's mods (horizon-joint-dev): the console command
// `pct_selftest run [filter]` sends machine reports down the same console path RustCraft uses,
// credits them to a pretend owner who is online only for this command, and compares each branch's
// experience with what data/tree.json says it should be. The result goes to
// selftest/path_of_craft_tree.json in the server folder:
//   { "mod": "path_of_craft_tree", "done": true, "passed": [...], "failed": [{ "name", "why" }] }
// Nothing is saved for the pretend owner. Every block position the scenarios name lies in the mod's
// part of the bench world (z -1000..0, y 100 and up), and the marks they leave are removed again.
use crate::game::{
    Game, pct_actions as actions, pct_glue as glue, pct_machine_rules as machine,
    pct_machines as machines, pct_mining as mining, pct_rules::Tree,
};
use crate::mod_api::NativeCommand;
use horizon_client::text::Text;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU32, Ordering},
};

/// The console command the joint bench runs (`selftest` in its mods.json).
pub const COMMAND: &str = "pct_selftest";
const MOD_ID: &str = "path_of_craft_tree";
const RESULT_DIR: &str = "selftest";
const DIMENSION: &str = "minecraft:overworld";
const AREA_X: i32 = 500;
const AREA_Y: i32 = 150;
const AREA_Z: i32 = -500;
/// Rows of positions a run may use before it wraps: runs in one server lifetime get fresh spots,
/// because a built spot pays only once a day.
const RUN_ROWS: u32 = 200;
const ENTITY_BASE: u32 = 2_000_000_000;
const TOLERANCE: f64 = 1e-6;

/// Each run moves to fresh positions and entity ids: the day-long build rule and the cows' rest
/// remember earlier runs.
static RUNS: AtomicU32 = AtomicU32::new(0);

pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op(&format!("{COMMAND} <args:text>"), 4));
}

pub fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let Some(args) = command
        .strip_prefix(COMMAND)
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    else {
        return;
    };
    *ran = true;
    let mut words = args.split_whitespace();
    match words.next() {
        Some("run") => run(game, words.next().unwrap_or("")),
        _ => game.native_feedback(Text::literal(format!("usage: {COMMAND} run [filter]"))),
    }
}

/// One check: the reports to send and the experience each branch should gain from them.
struct Scenario {
    name: &'static str,
    reports: Vec<String>,
    expected: BTreeMap<String, f64>,
}

fn run(game: &mut Game<'_>, filter: &str) {
    let Some(loaded) = glue::current_tree() else {
        game.native_feedback(Text::literal(format!("{COMMAND}: the tree is not loaded")));
        return;
    };
    let tree = &loaded.tree;
    let run_no = RUNS.fetch_add(1, Ordering::Relaxed);
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    for scenario in scenarios(tree, run_no)
        .into_iter()
        .filter(|s| s.name.contains(filter))
    {
        match check(game, tree, &scenario) {
            Ok(()) => passed.push(scenario.name),
            Err(why) => failed.push(serde_json::json!({ "name": scenario.name, "why": why })),
        }
    }
    glue::detach_test_player();
    actions::forget_player(glue::SELFTEST_PLAYER);
    let summary = format!(
        "{COMMAND}: {} passed, {} failed",
        passed.len(),
        failed.len()
    );
    let result =
        serde_json::json!({ "mod": MOD_ID, "done": true, "passed": passed, "failed": failed });
    let line = match write_result(game, &result) {
        Ok(path) => format!("{summary}; {path}"),
        Err(error) => format!("{summary}; result not written: {error}"),
    };
    log::info!("path_of_craft_tree: {line}");
    game.native_feedback(Text::literal(line));
}

/// Runs one scenario on a fresh profile and compares every branch that changed or should have.
fn check(game: &mut Game<'_>, tree: &Tree, scenario: &Scenario) -> Result<(), String> {
    glue::attach_test_player(tree);
    actions::start_test_visits(glue::SELFTEST_PLAYER);
    let before = glue::test_player_xp();
    for report in &scenario.reports {
        let mut ran = false;
        let line = format!("{} {report}", machine::COMMAND);
        machines::on_console_command(game, &line, &mut ran);
        if !ran {
            return Err(format!("the command was not taken: {line}"));
        }
    }
    let after = glue::test_player_xp();
    let mut branches: Vec<&String> = after.keys().chain(scenario.expected.keys()).collect();
    branches.sort();
    branches.dedup();
    let wrong: Vec<String> = branches
        .into_iter()
        .filter_map(|branch| {
            let gained = after.get(branch).copied().unwrap_or(0.0)
                - before.get(branch).copied().unwrap_or(0.0);
            let wanted = scenario.expected.get(branch).copied().unwrap_or(0.0);
            ((gained - wanted).abs() > TOLERANCE)
                .then(|| format!("{branch} gained {gained:.3}, expected {wanted:.3}"))
        })
        .collect();
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("; "))
    }
}

fn write_result(game: &Game<'_>, result: &serde_json::Value) -> Result<String, String> {
    let world = game
        .native_world_dir()
        .ok_or("no world folder on this side")?;
    let server = world.parent().ok_or("the world folder has no parent")?;
    let path = server.join(RESULT_DIR).join(format!("{MOD_ID}.json"));
    let bytes = serde_json::to_vec_pretty(result).map_err(|e| e.to_string())?;
    glue::write_in_folder(&path, &bytes)?;
    Ok(path.display().to_string())
}

/// The base experience of `action` and the branch it pays, from the tree.
fn base(tree: &Tree, action: &str) -> (String, f64) {
    tree.xp_for_action(action)
        .map(|(branch, value)| (branch.to_owned(), value))
        .unwrap_or_default()
}

fn health(tree: &Tree, kind: &str) -> f64 {
    tree.number("mob_health", kind).unwrap_or(20.0)
}

fn gains(pairs: impl IntoIterator<Item = (String, f64)>) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    for (branch, xp) in pairs {
        *map.entry(branch).or_insert(0.0) += xp;
    }
    map
}

/// What breaking `block` with `tool` pays a machine's owner, the way mining.rs works it out.
fn break_gain(tree: &Tree, block: &str, tool: machine::Tool) -> BTreeMap<String, f64> {
    let share = tree.machine_share;
    let Some(table) = mining::block_table(block) else {
        return BTreeMap::new();
    };
    let Some((branch, amount)) = tree.xp_for_break(&table, tool.item()) else {
        return BTreeMap::new();
    };
    let cross = mining::break_cross(tree, &table, 0)
        .map(|(branch, xp)| (branch, xp * share * tree.cross_share));
    gains(std::iter::once((branch.to_owned(), amount * share)).chain(cross))
}

fn scenarios(tree: &Tree, run_no: u32) -> Vec<Scenario> {
    let owner = format!("{:032x}", glue::SELFTEST_PLAYER);
    let share = tree.machine_share;
    let row = AREA_Z - (run_no % RUN_ROWS) as i32 * 4;
    let at = |dx: i32| format!("{DIMENSION} {} {AREA_Y} {row}", AREA_X + dx);
    let entity = |n: u32| ENTITY_BASE + run_no * 16 + n;
    let one = |action: &str, times: f64| {
        let (branch, value) = base(tree, action);
        gains([(branch, value * times * share)])
    };
    let none = BTreeMap::new;
    let mut list = vec![
        Scenario {
            name: "smelt_ingots_pay_the_smith",
            reports: vec![format!("smelt {owner} minecraft:iron_ingot 4")],
            expected: one("act:ingot", 4.0),
        },
        Scenario {
            name: "smelt_of_other_items_pays_nothing",
            reports: vec![format!("smelt {owner} minecraft:redstone 4")],
            expected: none(),
        },
        Scenario {
            name: "craft_pays_per_craft",
            reports: vec![format!("craft {owner} minecraft:paper minecraft:paper 9 3")],
            expected: one("act:craft", 3.0),
        },
        Scenario {
            name: "break_of_natural_ore_pays_the_miner",
            reports: vec![format!(
                "break {owner} {} minecraft:iron_ore pickaxe",
                at(0)
            )],
            expected: break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
        },
        Scenario {
            name: "break_with_the_wrong_tool_pays_nothing",
            reports: vec![format!("break {owner} {} minecraft:iron_ore axe", at(1))],
            expected: none(),
        },
        Scenario {
            name: "placed_block_pays_only_after_its_mark_is_gone",
            reports: vec![
                format!("place {} minecraft:iron_ore", at(2)),
                format!("break {owner} {} minecraft:iron_ore pickaxe", at(2)),
                format!("break {owner} {} minecraft:iron_ore pickaxe", at(2)),
            ],
            expected: break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
        },
        Scenario {
            name: "turret_kill_pays_the_archer",
            reports: vec![format!("kill {owner} {} minecraft:zombie", entity(0))],
            expected: one("act:shot", health(tree, "minecraft:zombie")),
        },
        Scenario {
            name: "player_shot_pays_in_full",
            reports: vec![format!("shot {owner} {} minecraft:zombie", entity(1))],
            expected: {
                let (branch, value) = base(tree, "act:shot");
                gains([(branch, value * health(tree, "minecraft:zombie"))])
            },
        },
        Scenario {
            name: "slaughter_pays_husbandry_only",
            reports: vec![format!("slaughter {owner} {} minecraft:cow", entity(2))],
            expected: gains([(
                "husbandry".to_owned(),
                health(tree, "minecraft:cow") * share * tree.cross_share,
            )]),
        },
        Scenario {
            name: "slaughter_of_a_monster_pays_nothing",
            reports: vec![format!("slaughter {owner} {} minecraft:zombie", entity(3))],
            expected: none(),
        },
        Scenario {
            name: "fish_and_treasure_pay_the_fisher",
            reports: vec![
                format!("fish {owner} minecraft:cod 2"),
                format!("fish {owner} minecraft:saddle 1"),
            ],
            expected: {
                let mut both = one("act:fish", 2.0);
                for (branch, xp) in one("act:treasure", 1.0) {
                    *both.entry(branch).or_insert(0.0) += xp;
                }
                both
            },
        },
        Scenario {
            name: "a_milked_cow_rests",
            reports: vec![
                format!("milk {owner} {}", entity(4)),
                format!("milk {owner} {}", entity(4)),
            ],
            expected: one("act:milk", 1.0),
        },
        Scenario {
            name: "build_pays_once_and_marks_the_block",
            reports: vec![
                format!("build {owner} {} minecraft:stone_bricks", at(3)),
                format!("build {owner} {} minecraft:stone_bricks", at(3)),
                format!("break {owner} {} minecraft:stone_bricks pickaxe", at(3)),
            ],
            expected: one("act:build", 1.0),
        },
        Scenario {
            name: "finds_pay_once",
            reports: vec![
                format!("chunk {owner} {DIMENSION} 31 {}", row >> 4),
                format!("chunk {owner} {DIMENSION} 31 {}", row >> 4),
                format!("biome {owner} minecraft:mushroom_fields"),
                format!("biome {owner} minecraft:mushroom_fields"),
                format!("structure {owner} minecraft:ancient_city"),
            ],
            expected: {
                let mut all = BTreeMap::new();
                for action in ["act:chunk", "act:biome", "act:structure"] {
                    for (branch, xp) in one(action, 1.0) {
                        *all.entry(branch).or_insert(0.0) += xp;
                    }
                }
                all
            },
        },
        Scenario {
            name: "broken_reports_pay_nothing",
            reports: vec![
                format!("brew {owner} 0"),
                format!("dance {owner} 1"),
                format!("smelt nobody minecraft:iron_ingot 2"),
            ],
            expected: none(),
        },
    ];
    for (word, action, per) in [
        ("repair", "act:repair", 1.0),
        ("breed", "act:breed", 1.0),
        ("shear", "act:shear", 1.0),
        ("honey", "act:honey", 1.0),
        ("brush", "act:brush", 1.0),
        ("brew", "act:brew", 3.0),
        ("enchant", "act:enchant", 1.0),
    ] {
        list.push(Scenario {
            name: work_name(word),
            reports: vec![format!("{word} {owner} 2")],
            expected: one(action, 2.0 * per),
        });
    }
    list
}

fn work_name(word: &str) -> &'static str {
    match word {
        "repair" => "work_repair_pays",
        "breed" => "work_breed_pays",
        "shear" => "work_shear_pays",
        "honey" => "work_honey_pays",
        "brush" => "work_brush_pays",
        "brew" => "work_brew_pays_three_bottles",
        _ => "work_enchant_pays_per_level",
    }
}
