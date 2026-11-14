// Self-test for the joint bench of the pack's mods (horizon-joint-dev): the console command
// `pct_selftest run [filter]` sends machine reports down the same console path RustCraft uses,
// credits them to a pretend owner who is online only for this command, and compares each branch's
// experience with what data/tree.json says it should be. The result goes to
// selftest/path_of_craft_tree.json in the server folder:
//   { "mod": "path_of_craft_tree", "done": true, "passed": [...], "failed": [{ "name", "why" }] }
// The team scenarios put several pretend owners online at once and check that each earns only its
// own experience, that shared world state (built spots, a cow's rest, placed marks) holds for all
// of them and that per-player state (visited chunks, one-time finds) stays per player.
// Nothing is saved for the pretend owners. Every block position the scenarios name lies in the mod's
// part of the bench world (z -1000..0, y 100 and up), and the marks they leave are removed again.
use crate::game::{
    Game, pct_actions as actions,
    pct_fx_roam_rules::{SPAWNER_TAG, SPAWNER_XP_SHARE},
    pct_glue as glue, pct_machine_rules as machine, pct_machines as machines, pct_mining as mining,
    pct_rules::{Profile, Tree},
};
use crate::mod_api::NativeCommand;
use horizon_client::text::Text;
use horizon_world::nbt::{JavaString, Tag};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
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
/// because a built spot pays only once a day. 125 rows of 4 blocks keep every row inside the mod's
/// part of the bench world (z down to -996).
const RUN_ROWS: u32 = 125;
const ROW_STEP: i32 = 4;
/// Entity ids one run may use; the next run starts after them.
const ENTITIES_PER_RUN: u32 = 16;
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
        Some("summary") => show_summaries(game),
        Some("visits") => {
            let name = words.next().unwrap_or("");
            let extra = words.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            measure_visits(game, name, extra);
        }
        _ => game.native_feedback(Text::literal(format!(
            "usage: {COMMAND} run [filter] | {COMMAND} summary | {COMMAND} visits <player> <extra chunks>"
        ))),
    }
}

/// Sizes the visited-chunks store with a test bot: adds `extra` made-up chunks to an online player's
/// set and reports the set's size, its file and the time to save and to read it back.
fn measure_visits(game: &mut Game<'_>, name: &str, extra: usize) {
    let player = game
        .native_server_players()
        .into_iter()
        .find(|p| p.name == name)
        .map(|p| p.uuid);
    let line = match player {
        None => format!("{COMMAND}: {name} is not online"),
        Some(uuid) => match actions::measure_visits(game, uuid, extra) {
            Ok(m) => format!(
                "{COMMAND}: visits {name}: {} chunks, file {} bytes ({:.1} per chunk), save {:.1} ms, load {:.1} ms",
                m.chunks,
                m.file_bytes,
                m.file_bytes as f64 / m.chunks.max(1) as f64,
                m.save.as_secs_f64() * 1000.0,
                m.load.as_secs_f64() * 1000.0
            ),
            Err(why) => format!("{COMMAND}: visits {name}: {why}"),
        },
    };
    log::info!("path_of_craft_tree: {line}");
    game.native_feedback(Text::literal(line));
}

/// Prints the combat summary each online player carries, as another mod reads it.
fn show_summaries(game: &mut Game<'_>) {
    let players = game.native_server_players();
    if players.is_empty() {
        game.native_feedback(Text::literal(format!("{COMMAND}: nobody is online")));
    }
    for player in players {
        let summary = game
            .native_player_persistent_data(player.uuid)
            .and_then(|data| match data.get(glue::SUMMARY_KEY) {
                Some(Tag::String(text)) => Some(text.to_string_lossy()),
                _ => None,
            })
            .unwrap_or_else(|| "(none)".to_owned());
        let line = format!("{}: {} {summary}", player.name, glue::SUMMARY_KEY);
        log::info!("path_of_craft_tree: {line}");
        game.native_feedback(Text::literal(line));
    }
}

/// The scenarios that need the tagged test mob.
const TAGGED_SCENARIOS: [&str; 3] = [
    "a_pct_xp_tag_scales_a_turret_kill",
    "a_pct_xp_tag_scales_a_slaughter",
    "a_spawner_tag_kept_with_the_mob_cuts_a_kill",
];
/// Where the tagged test mob stands: in the mod's part of the bench world, by the world spawn; the
/// run holds its chunk loaded while it needs it. It floats without AI and is removed after the run.
const TAGGED_MOB_POS: [f64; 3] = [8.5, 101.0, -8.5];
const TAGGED_MOB_KIND: &str = "minecraft:cow";
const TAGGED_MOB_TAG: &str = "pct.xp145";
const TAGGED_MULTIPLIER: f64 = 1.45;
/// A SoulsCraft brute's health as SoulsCraft states it (data/mobs.json); the tree must agree.
const SOULSCRAFT_BRUTE_HEALTH: f64 = 60.0;

/// A cow carrying `tag`: `pct.xp145`, as SoulsCraft tags the mobs it makes stronger, or the
/// spawner tag, as a spawner mob read back from the world carries it. Its id, or None when the
/// game refused the summon.
fn summon_tagged_mob(game: &mut Game<'_>, tag: &str) -> Option<u32> {
    let nbt = Tag::compound([
        ("NoAI", Tag::Byte(1)),
        ("NoGravity", Tag::Byte(1)),
        ("Silent", Tag::Byte(1)),
        ("Invulnerable", Tag::Byte(1)),
        ("PersistenceRequired", Tag::Byte(1)),
        ("Tags", Tag::List(vec![Tag::String(JavaString::from(tag))])),
    ]);
    let Tag::Compound(nbt) = nbt else {
        return None;
    };
    game.native_summon(DIMENSION, TAGGED_MOB_KIND, TAGGED_MOB_POS, nbt)
}

/// The kills of the tagged mob, reported by machines, each at 145%: a turret's, whose report calls
/// the cow a zombie on purpose (the tree must pay by the mob's own kind, as for a content mod's mob
/// built on a vanilla template), and a slaughterer's. A turret's kill of the cow with the spawner
/// tag pays the spawner share: the tag, not the mod's memory, says where the mob came from, so the
/// cut holds after a restart.
fn tagged_scenarios(tree: &Tree, mob: u32, spawner_mob: Option<u32>) -> Vec<Scenario> {
    let owner = format!("{:032x}", glue::SELFTEST_PLAYER);
    let share = tree.machine_share;
    let (archery, shot) = base(tree, "act:shot");
    let spawner_kill = spawner_mob.map(|spawner_mob| Solo {
        name: TAGGED_SCENARIOS[2],
        reports: vec![format!("kill {owner} {spawner_mob} {TAGGED_MOB_KIND}")],
        expected: gains([(
            archery.clone(),
            shot * health(tree, TAGGED_MOB_KIND) * share * SPAWNER_XP_SHARE,
        )]),
    });
    vec![
        Solo {
            name: TAGGED_SCENARIOS[0],
            reports: vec![format!("kill {owner} {mob} minecraft:zombie")],
            expected: gains([(
                archery,
                shot * health(tree, TAGGED_MOB_KIND) * share * TAGGED_MULTIPLIER,
            )]),
        },
        Solo {
            name: TAGGED_SCENARIOS[1],
            reports: vec![format!("slaughter {owner} {mob} {TAGGED_MOB_KIND}")],
            expected: gains([(
                "husbandry".to_owned(),
                health(tree, TAGGED_MOB_KIND) * share * tree.cross_share * TAGGED_MULTIPLIER,
            )]),
        },
    ]
    .into_iter()
    .chain(spawner_kill)
    .map(Scenario::from)
    .collect()
}

/// Experience by branch.
type Gains = BTreeMap<String, f64>;

/// A check with the main pretend owner alone: the reports to send and the experience each branch
/// should gain from them.
struct Solo {
    name: &'static str,
    reports: Vec<String>,
    expected: Gains,
}

/// One check: the reports to send and what each pretend owner online for it should gain. An owner
/// expected to gain nothing must keep its fresh profile whole.
struct Scenario {
    name: &'static str,
    reports: Vec<String>,
    expected: Vec<(u128, Gains)>,
    /// An owner whose branch must reach a new level during the check.
    levels_up: Option<(u128, String)>,
}

impl From<Solo> for Scenario {
    fn from(solo: Solo) -> Self {
        Scenario {
            name: solo.name,
            reports: solo.reports,
            expected: vec![(glue::SELFTEST_PLAYER, solo.expected)],
            levels_up: None,
        }
    }
}

/// The pretend owners a check put online. Dropping it takes every pretend owner offline again and
/// forgets their per-player state, also when the check stops half-way.
struct Online;

impl Online {
    fn attach(tree: &Tree, owners: impl IntoIterator<Item = u128>) -> Self {
        for owner in owners {
            glue::attach_test_player(tree, owner);
            actions::start_test_visits(owner);
        }
        Online
    }
}

impl Drop for Online {
    fn drop(&mut self) {
        for owner in glue::SELFTEST_PLAYERS {
            glue::detach_test_player(owner);
            actions::forget_player(owner);
        }
    }
}

/// Positions and entity ids of one run: each run moves to a fresh row and fresh ids.
struct Spots {
    run_no: u32,
    row: i32,
}

impl Spots {
    fn new(run_no: u32) -> Self {
        Spots {
            run_no,
            row: AREA_Z - (run_no % RUN_ROWS) as i32 * ROW_STEP,
        }
    }

    /// A block position `dx` along this run's row, as a report names it.
    fn at(&self, dx: i32) -> String {
        format!("{DIMENSION} {} {AREA_Y} {}", AREA_X + dx, self.row)
    }

    /// The chunk row this run's positions lie in.
    fn chunk_z(&self) -> i32 {
        self.row >> 4
    }

    fn entity(&self, n: u32) -> u32 {
        ENTITY_BASE + self.run_no * ENTITIES_PER_RUN + n
    }
}

fn run(game: &mut Game<'_>, filter: &str) {
    let Some(loaded) = glue::current_tree() else {
        game.native_feedback(Text::literal(format!("{COMMAND}: the tree is not loaded")));
        return;
    };
    let tree = &loaded.tree;
    let run_no = RUNS.fetch_add(1, Ordering::Relaxed);
    let mut outcome = Outcome::default();
    for scenario in scenarios(tree, run_no)
        .into_iter()
        .filter(|s| s.name.contains(filter))
    {
        outcome.record(scenario.name, check(game, tree, &scenario));
    }
    if TAGGED_SCENARIOS.iter().any(|name| name.contains(filter)) {
        // SoulsCraft's contract needs a real mob, and a fresh world with nobody on has no chunk
        // loaded: hold one and finish on the server ticks once it is in (`on_server_tick`).
        game.native_force_chunk(CHUNK_OWNER, tagged_chunk(), true);
        let line = format!("{COMMAND}: waiting for a loaded chunk for the tagged-mob scenarios");
        let _ = write_result(game, &outcome.json(false));
        *pending() = Some(Pending {
            outcome,
            filter: filter.to_owned(),
            ticks: 0,
        });
        game.native_feedback(Text::literal(line));
        return;
    }
    finish(game, outcome);
}

/// The scenarios' results so far.
#[derive(Default)]
struct Outcome {
    passed: Vec<&'static str>,
    failed: Vec<serde_json::Value>,
}

impl Outcome {
    fn record(&mut self, name: &'static str, result: Result<(), String>) {
        match result {
            Ok(()) => self.passed.push(name),
            Err(why) => self
                .failed
                .push(serde_json::json!({ "name": name, "why": why })),
        }
    }

    fn json(&self, done: bool) -> serde_json::Value {
        serde_json::json!({ "mod": MOD_ID, "done": done, "passed": self.passed, "failed": self.failed })
    }
}

/// A run waiting on the server ticks for the tagged mob's chunk.
struct Pending {
    outcome: Outcome,
    filter: String,
    ticks: u32,
}

/// Who holds the test chunk (`native_force_chunk`).
const CHUNK_OWNER: &str = "path_of_craft_tree:selftest";
/// How long a run waits for the chunk; the bench sprints ticks, so this is a fraction of a second.
const MAX_WAIT_TICKS: u32 = 600;

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn tagged_chunk() -> [i32; 2] {
    [
        (TAGGED_MOB_POS[0].floor() as i32) >> 4,
        (TAGGED_MOB_POS[2].floor() as i32) >> 4,
    ]
}

/// Server tick: a waiting run tries for its tagged mob, and finishes once it has it or gives up.
pub fn on_server_tick(game: &mut Game<'_>) {
    let Some(mut run) = pending().take() else {
        return;
    };
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    match summon_tagged_mob(game, TAGGED_MOB_TAG) {
        Some(mob) => {
            let spawner_mob = summon_tagged_mob(game, SPAWNER_TAG);
            for scenario in tagged_scenarios(tree, mob, spawner_mob)
                .into_iter()
                .filter(|s| s.name.contains(run.filter.as_str()))
            {
                let result = check(game, tree, &scenario);
                run.outcome.record(scenario.name, result);
            }
            game.native_remove_mob(DIMENSION, mob);
            match spawner_mob {
                Some(spawner_mob) => {
                    game.native_remove_mob(DIMENSION, spawner_mob);
                }
                None if TAGGED_SCENARIOS[2].contains(run.filter.as_str()) => {
                    let why = "the game refused the cow with the spawner tag".to_owned();
                    run.outcome.record(TAGGED_SCENARIOS[2], Err(why));
                }
                None => {}
            }
        }
        None if run.ticks < MAX_WAIT_TICKS => {
            run.ticks += 1;
            *pending() = Some(run);
            return;
        }
        None => {
            let why = format!(
                "no test mob: the chunk at {TAGGED_MOB_POS:?} did not load in {MAX_WAIT_TICKS} ticks"
            );
            run.outcome.record(TAGGED_SCENARIOS[0], Err(why));
        }
    }
    game.native_force_chunk(CHUNK_OWNER, tagged_chunk(), false);
    finish(game, run.outcome);
}

/// Writes the final result and answers on the console.
fn finish(game: &mut Game<'_>, outcome: Outcome) {
    let summary = format!(
        "{COMMAND}: {} passed, {} failed",
        outcome.passed.len(),
        outcome.failed.len()
    );
    let line = match write_result(game, &outcome.json(true)) {
        Ok(path) => format!("{summary}; {path}"),
        Err(error) => format!("{summary}; result not written: {error}"),
    };
    log::info!("path_of_craft_tree: {line}");
    game.native_feedback(Text::literal(line));
}

/// Runs one scenario with its owners online on fresh profiles and compares, for each owner, every
/// branch that changed or should have. All pretend owners go offline again when it returns.
fn check(game: &mut Game<'_>, tree: &Tree, scenario: &Scenario) -> Result<(), String> {
    let _online = Online::attach(tree, scenario.expected.iter().map(|(owner, _)| *owner));
    let profile = |owner: u128| glue::test_player_profile(owner).unwrap_or_default();
    let before: Vec<Profile> = scenario
        .expected
        .iter()
        .map(|(owner, _)| profile(*owner))
        .collect();
    for report in &scenario.reports {
        if let Some(at) = report.strip_prefix(EXPLODE_STEP) {
            explode(game, at)?;
            continue;
        }
        let mut ran = false;
        let line = format!("{} {report}", machine::COMMAND);
        machines::on_console_command(game, &line, &mut ran);
        if !ran {
            return Err(format!("the command was not taken: {line}"));
        }
    }
    let several = scenario.expected.len() > 1;
    let mut wrong = Vec::new();
    for ((owner, expected), before) in scenario.expected.iter().zip(&before) {
        let who = if several {
            format!("{}: ", owner_name(*owner))
        } else {
            String::new()
        };
        let Some(after) = glue::test_player_profile(*owner) else {
            wrong.push(format!("{who}went offline"));
            continue;
        };
        wrong.extend(
            xp_mismatches(&before.xp, &after.xp, expected)
                .into_iter()
                .map(|line| format!("{who}{line}")),
        );
        if expected.is_empty() && after != *before {
            wrong.push(format!("{who}the profile changed, though nothing was earned"));
        }
    }
    if let Some((owner, branch)) = &scenario.levels_up {
        let level = |p: &Profile| tree.branch_level(p, branch);
        let was = level(&tree.new_profile());
        let now = level(&profile(*owner));
        if now <= was {
            wrong.push(format!(
                "{}: {branch} stayed at level {now}, expected a level-up",
                owner_name(*owner)
            ));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("; "))
    }
}

/// A scenario step the selftest runs itself instead of reporting it: an explosion removes the
/// block at the position that follows, as `explosion_hit_block` reports each block a blast takes.
const EXPLODE_STEP: &str = "explode ";

/// Runs an explosion step at `at` (`minecraft:overworld x y z`): the block's mark goes as on the
/// server tick after a real blast.
fn explode(game: &Game<'_>, at: &str) -> Result<(), String> {
    let mut words = at.split_whitespace();
    let dimension = words.next().filter(|word| *word == DIMENSION);
    let coords: Vec<i32> = words.filter_map(|word| word.parse().ok()).collect();
    let (Some(_), [x, y, z]) = (dimension, coords.as_slice()) else {
        return Err(format!("not an explosion step: {at}"));
    };
    mining::on_explosion_block(DIMENSION, [*x, *y, *z]);
    mining::apply_explosions(game);
    Ok(())
}

/// The branches whose gain from `before` to `after` differs from `expected`.
fn xp_mismatches(before: &Gains, after: &Gains, expected: &Gains) -> Vec<String> {
    let mut branches: Vec<&String> = after.keys().chain(expected.keys()).collect();
    branches.sort();
    branches.dedup();
    branches
        .into_iter()
        .filter_map(|branch| {
            let gained = after.get(branch).copied().unwrap_or(0.0)
                - before.get(branch).copied().unwrap_or(0.0);
            let wanted = expected.get(branch).copied().unwrap_or(0.0);
            ((gained - wanted).abs() > TOLERANCE)
                .then(|| format!("{branch} gained {gained:.3}, expected {wanted:.3}"))
        })
        .collect()
}

/// How a failure names a pretend owner: by its place in `glue::SELFTEST_PLAYERS`.
fn owner_name(owner: u128) -> String {
    let index = glue::SELFTEST_PLAYERS
        .iter()
        .position(|p| *p == owner)
        .unwrap_or(usize::MAX);
    match index {
        0 => "main owner".to_owned(),
        1 => "owner A".to_owned(),
        2 => "owner B".to_owned(),
        3 => "owner C".to_owned(),
        _ => format!("{owner:032x}"),
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

/// What a machine's `action` done `times` over pays its owner.
fn machine_gain(tree: &Tree, action: &str, times: f64) -> Gains {
    let (branch, value) = base(tree, action);
    gains([(branch, value * times * tree.machine_share)])
}

/// Adds several owners' worth of gains into one.
fn sum(parts: impl IntoIterator<Item = Gains>) -> Gains {
    gains(parts.into_iter().flatten())
}

fn scenarios(tree: &Tree, run_no: u32) -> Vec<Scenario> {
    let spots = Spots::new(run_no);
    let mut list: Vec<Scenario> = solo_scenarios(tree, &spots)
        .into_iter()
        .map(Scenario::from)
        .collect();
    list.extend(team_scenarios(tree, &spots));
    list
}

fn solo_scenarios(tree: &Tree, spots: &Spots) -> Vec<Solo> {
    let owner = format!("{:032x}", glue::SELFTEST_PLAYER);
    let share = tree.machine_share;
    let row = spots.row;
    let at = |dx: i32| spots.at(dx);
    let entity = |n: u32| spots.entity(n);
    let one = |action: &str, times: f64| machine_gain(tree, action, times);
    let none = BTreeMap::new;
    let mut list = vec![
        Solo {
            name: "smelt_ingots_pay_the_smith",
            reports: vec![format!("smelt {owner} minecraft:iron_ingot 4")],
            expected: one("act:ingot", 4.0),
        },
        Solo {
            name: "smelt_of_other_items_pays_nothing",
            reports: vec![format!("smelt {owner} minecraft:redstone 4")],
            expected: none(),
        },
        Solo {
            name: "craft_pays_per_craft",
            reports: vec![format!("craft {owner} minecraft:paper minecraft:paper 9 3")],
            expected: one("act:craft", 3.0),
        },
        Solo {
            name: "break_of_natural_ore_pays_the_miner",
            reports: vec![format!(
                "break {owner} {} minecraft:iron_ore pickaxe",
                at(0)
            )],
            expected: break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
        },
        Solo {
            name: "break_with_the_wrong_tool_pays_nothing",
            reports: vec![format!("break {owner} {} minecraft:iron_ore axe", at(1))],
            expected: none(),
        },
        Solo {
            name: "placed_block_pays_only_after_its_mark_is_gone",
            reports: vec![
                format!("place {} minecraft:iron_ore", at(2)),
                format!("break {owner} {} minecraft:iron_ore pickaxe", at(2)),
                format!("break {owner} {} minecraft:iron_ore pickaxe", at(2)),
            ],
            expected: break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
        },
        Solo {
            // A blast takes the placed block and its mark; the natural ore later at that spot pays.
            name: "an_exploded_placed_block_loses_its_mark",
            reports: vec![
                format!("place {} minecraft:iron_ore", at(6)),
                format!("{EXPLODE_STEP}{}", at(6)),
                format!("break {owner} {} minecraft:iron_ore pickaxe", at(6)),
            ],
            expected: break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
        },
        Solo {
            name: "turret_kill_pays_the_archer",
            reports: vec![format!("kill {owner} {} minecraft:zombie", entity(0))],
            expected: one("act:shot", health(tree, "minecraft:zombie")),
        },
        Solo {
            // SoulsCraft's mobs are in the health table (agreed 2026-11-13): a brute is worth 60.
            name: "a_soulscraft_brute_pays_its_own_health",
            reports: vec![format!("kill {owner} {} soulscraft:brute", entity(5))],
            expected: one("act:shot", SOULSCRAFT_BRUTE_HEALTH),
        },
        Solo {
            name: "player_shot_pays_in_full",
            reports: vec![format!("shot {owner} {} minecraft:zombie", entity(1))],
            expected: {
                let (branch, value) = base(tree, "act:shot");
                gains([(branch, value * health(tree, "minecraft:zombie"))])
            },
        },
        Solo {
            name: "slaughter_pays_husbandry_only",
            reports: vec![format!("slaughter {owner} {} minecraft:cow", entity(2))],
            expected: gains([(
                "husbandry".to_owned(),
                health(tree, "minecraft:cow") * share * tree.cross_share,
            )]),
        },
        Solo {
            name: "slaughter_of_a_monster_pays_nothing",
            reports: vec![format!("slaughter {owner} {} minecraft:zombie", entity(3))],
            expected: none(),
        },
        Solo {
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
        Solo {
            name: "a_milked_cow_rests",
            reports: vec![
                format!("milk {owner} {}", entity(4)),
                format!("milk {owner} {}", entity(4)),
            ],
            expected: one("act:milk", 1.0),
        },
        Solo {
            name: "build_pays_once_and_marks_the_block",
            reports: vec![
                format!("build {owner} {} minecraft:stone_bricks", at(3)),
                format!("build {owner} {} minecraft:stone_bricks", at(3)),
                format!("break {owner} {} minecraft:stone_bricks pickaxe", at(3)),
            ],
            expected: one("act:build", 1.0),
        },
        Solo {
            // Breaking a block where building paid makes the breaker's next paying placement
            // pay nothing: "place, break, place next to it" earns once (design.md, 14).
            name: "build_after_breaking_a_paid_block_pays_nothing",
            reports: vec![
                format!("build {owner} {} minecraft:stone_bricks", at(6)),
                format!("break {owner} {} minecraft:stone_bricks pickaxe", at(6)),
                format!("build {owner} {} minecraft:stone_bricks", at(7)),
                format!("build {owner} {} minecraft:oak_stairs", at(8)),
            ],
            expected: sum([one("act:build", 1.0), one("act:deco", 1.0)]),
        },
        Solo {
            name: "brew_without_bottles_pays_three_a_brew",
            reports: vec![format!("brew {owner} 2")],
            expected: one("act:brew", 6.0),
        },
        Solo {
            name: "brew_pays_the_bottles_it_names",
            reports: vec![format!("brew {owner} 2 4")],
            expected: one("act:brew", 4.0),
        },
        Solo {
            name: "brew_with_impossible_bottles_pays_nothing",
            reports: vec![format!("brew {owner} 2 7"), format!("brew {owner} 2 1")],
            expected: none(),
        },
        Solo {
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
        Solo {
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
        ("enchant", "act:enchant", 1.0),
    ] {
        list.push(Solo {
            name: work_name(word),
            reports: vec![format!("{word} {owner} 2")],
            expected: one(action, 2.0 * per),
        });
    }
    list
}

/// The most ingots the level-up scenario may report at once, should the curve ever be that steep.
const MAX_LEVEL_UP_INGOTS: u32 = 1_000_000;

/// How many ingots a machine must smelt for its owner's smithing branch to reach level 1.
fn ingots_to_level_up(tree: &Tree) -> u32 {
    let (_, per_ingot) = base(tree, "act:ingot");
    let per_ingot = per_ingot * tree.machine_share;
    if per_ingot <= 0.0 {
        return 1;
    }
    (1..MAX_LEVEL_UP_INGOTS)
        .find(|n| tree.level_progress(per_ingot * f64::from(*n)).0 >= 1)
        .unwrap_or(MAX_LEVEL_UP_INGOTS)
}

/// Several pretend owners earning at once. The main owner is online in each but sent nothing, so
/// any experience that leaks to a bystander shows up too.
fn team_scenarios(tree: &Tree, spots: &Spots) -> Vec<Scenario> {
    let [main, a, b, c] = glue::SELFTEST_PLAYERS;
    let id = |owner: u128| format!("{owner:032x}");
    let (ida, idb, idc) = (id(a), id(b), id(c));
    let at = |dx: i32| spots.at(dx);
    let entity = |n: u32| spots.entity(n);
    let chunk = format!("{DIMENSION} 32 {}", spots.chunk_z());
    let one = |action: &str, times: f64| machine_gain(tree, action, times);
    let none = BTreeMap::new;
    let owners = |ga: Gains, gb: Gains, gc: Gains| vec![(main, none()), (a, ga), (b, gb), (c, gc)];
    let finds = || sum(["act:chunk", "act:biome", "act:structure"].map(|act| one(act, 1.0)));
    let ingots = ingots_to_level_up(tree);
    vec![
        Scenario {
            name: "team_interleaved_reports_pay_each_owner_its_own",
            reports: vec![
                format!("smelt {ida} minecraft:iron_ingot 4"),
                format!("craft {idb} minecraft:paper minecraft:paper 9 3"),
                format!("fish {idc} minecraft:cod 2"),
                format!("fish {ida} minecraft:cod 1"),
                format!("smelt {idb} minecraft:iron_ingot 1"),
                format!("kill {idc} {} minecraft:zombie", entity(8)),
                format!("repair {ida} 2"),
            ],
            expected: owners(
                sum([one("act:ingot", 4.0), one("act:fish", 1.0), one("act:repair", 2.0)]),
                sum([one("act:craft", 3.0), one("act:ingot", 1.0)]),
                sum([
                    one("act:fish", 2.0),
                    one("act:shot", health(tree, "minecraft:zombie")),
                ]),
            ),
            levels_up: None,
        },
        Scenario {
            // The day-long build rule is the spot's, not the builder's.
            name: "team_a_built_spot_pays_no_other_owner_that_day",
            reports: vec![
                format!("build {ida} {} minecraft:stone_bricks", at(4)),
                format!("build {idb} {} minecraft:stone_bricks", at(4)),
                format!("build {idc} {} minecraft:stone_bricks", at(4)),
                format!("break {idb} {} minecraft:stone_bricks pickaxe", at(4)),
            ],
            expected: owners(one("act:build", 1.0), none(), none()),
            levels_up: None,
        },
        Scenario {
            // A placed block's mark is the world's: whoever's machine breaks it first gets nothing
            // and clears it, and the next break pays.
            name: "team_a_placed_mark_blocks_every_owners_break_pay",
            reports: vec![
                format!("place {} minecraft:iron_ore", at(5)),
                format!("break {idb} {} minecraft:iron_ore pickaxe", at(5)),
                format!("break {ida} {} minecraft:iron_ore pickaxe", at(5)),
            ],
            expected: owners(
                break_gain(tree, "minecraft:iron_ore", machine::Tool::Pickaxe),
                none(),
                none(),
            ),
            levels_up: None,
        },
        Scenario {
            // A broken paid block costs the breaker a placement, not the builder.
            name: "team_a_broken_paid_block_costs_only_the_breaker",
            reports: vec![
                format!("build {ida} {} minecraft:stone_bricks", at(9)),
                format!("break {idb} {} minecraft:stone_bricks pickaxe", at(9)),
                format!("build {ida} {} minecraft:stone_bricks", at(10)),
                format!("build {idb} {} minecraft:stone_bricks", at(11)),
                format!("build {idb} {} minecraft:stone_bricks", at(12)),
            ],
            expected: owners(one("act:build", 2.0), one("act:build", 1.0), none()),
            levels_up: None,
        },
        Scenario {
            // A cow's rest is the cow's: milked by A, it pays B nothing; B's own cow pays B.
            name: "team_a_milked_cow_rests_for_every_owner",
            reports: vec![
                format!("milk {ida} {}", entity(6)),
                format!("milk {idb} {}", entity(6)),
                format!("milk {idc} {}", entity(6)),
                format!("milk {idb} {}", entity(7)),
            ],
            expected: owners(one("act:milk", 1.0), one("act:milk", 1.0), none()),
            levels_up: None,
        },
        Scenario {
            // Visited chunks and one-time finds are each player's own.
            name: "team_each_owner_finds_the_same_chunk_and_biome_once",
            reports: vec![
                format!("chunk {ida} {chunk}"),
                format!("chunk {idb} {chunk}"),
                format!("chunk {ida} {chunk}"),
                format!("chunk {idb} {chunk}"),
                format!("biome {ida} minecraft:deep_dark"),
                format!("biome {idb} minecraft:deep_dark"),
                format!("biome {idb} minecraft:deep_dark"),
                format!("biome {ida} minecraft:deep_dark"),
                format!("structure {idb} minecraft:trial_chambers"),
                format!("structure {ida} minecraft:trial_chambers"),
            ],
            expected: owners(finds(), finds(), none()),
            levels_up: None,
        },
        Scenario {
            name: "team_one_owners_level_up_leaves_the_others_alone",
            reports: vec![
                format!("smelt {idb} minecraft:iron_ingot 1"),
                format!("smelt {ida} minecraft:iron_ingot {ingots}"),
                format!("smelt {idb} minecraft:iron_ingot 1"),
            ],
            expected: owners(
                one("act:ingot", f64::from(ingots)),
                one("act:ingot", 2.0),
                none(),
            ),
            levels_up: Some((a, base(tree, "act:ingot").0)),
        },
    ]
}

fn work_name(word: &str) -> &'static str {
    match word {
        "repair" => "work_repair_pays",
        "breed" => "work_breed_pays",
        "shear" => "work_shear_pays",
        "honey" => "work_honey_pays",
        "brush" => "work_brush_pays",
        _ => "work_enchant_pays_per_level",
    }
}
