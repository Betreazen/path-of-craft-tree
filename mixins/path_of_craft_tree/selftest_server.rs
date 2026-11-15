// Self-test scenarios for the server slice, run by `pct_selftest run` beside the machine-report
// ones (selftest.rs): the owner's multipliers, the admin commands on a pretend owner, the summary's
// all-branch fields and group shares between pretend owners. The pretend owners are online only
// inside a scenario and nothing of theirs is saved; settings and groups go back as they were.
use crate::game::{
    Game, pct_actions as actions,
    pct_commands::{self as commands, Caller},
    pct_glue as glue,
    pct_group_rules::Nearby,
    pct_groups::TestGroup,
    pct_machine_rules as machine, pct_machines as machines,
    pct_rules::{Profile, Tree, XpScale},
    pct_settings::{self as settings, TestSettings},
    pct_settings_rules::Settings,
};
use std::collections::BTreeMap;

const TOLERANCE: f64 = 1e-6;
const DIMENSION: &str = "minecraft:overworld";
const OTHER_DIMENSION: &str = "minecraft:the_nether";

type Gains = BTreeMap<String, f64>;
type Check = fn(&mut Game<'_>, &Tree) -> Result<(), String>;

const SCENARIOS: [(&str, Check); 5] = [
    ("server_settings_multiply_experience", settings_multiply),
    ("admin_commands_change_a_pretend_owner", admin_commands),
    ("summary_has_every_branch_and_keeps_v1", summary_fields),
    ("group_share_pays_near_members_once", group_share),
    (
        "group_share_skips_creative_far_and_other_dimension",
        group_skips,
    ),
];

/// The scenarios whose names contain `filter`, with their results. `tree` is the tree with the
/// default settings the self-test runs under.
pub fn run(
    game: &mut Game<'_>,
    tree: &Tree,
    filter: &str,
) -> Vec<(&'static str, Result<(), String>)> {
    SCENARIOS
        .iter()
        .filter(|(name, _)| name.contains(filter))
        .map(|(name, check)| (*name, check(game, tree)))
        .collect()
}

/// Pretend owners online for one scenario; offline again when dropped.
struct Online;

impl Online {
    fn attach(tree: &Tree, owners: &[u128]) -> Self {
        for owner in owners {
            glue::attach_test_player(tree, *owner);
            actions::start_test_visits(*owner);
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

fn profile(owner: u128) -> Profile {
    glue::test_player_profile(owner).unwrap_or_default()
}

fn id(owner: u128) -> String {
    format!("{owner:032x}")
}

/// Sends a machine report down the console path, as RustCraft does.
fn report(game: &mut Game<'_>, line: &str) -> Result<(), String> {
    let mut ran = false;
    machines::on_console_command(game, &format!("{} {line}", machine::COMMAND), &mut ran);
    if ran {
        Ok(())
    } else {
        Err(format!("the report was not taken: {line}"))
    }
}

/// Runs a `/pct` command as the console does.
fn admin(game: &mut Game<'_>, line: &str) -> Result<Vec<String>, String> {
    commands::run(game, &Caller::Console, line).ok_or_else(|| format!("not a /pct command: {line}"))
}

/// The branch and machine-paid experience of `action` done `times` over.
fn machine_gain(tree: &Tree, action: &str, times: f64) -> (String, f64) {
    let (branch, value) = tree.xp_for_action(action).unwrap_or(("", 0.0));
    (branch.to_owned(), value * times * tree.machine_share)
}

fn gains(pairs: impl IntoIterator<Item = (String, f64)>) -> Gains {
    let mut map = Gains::new();
    for (branch, xp) in pairs {
        *map.entry(branch).or_insert(0.0) += xp;
    }
    map
}

/// The branches whose gain from `before` to `after` is not `expected`.
fn mismatches(who: &str, before: &Profile, after: &Profile, expected: &Gains) -> Vec<String> {
    let mut branches: Vec<&String> = after.xp.keys().chain(expected.keys()).collect();
    branches.sort();
    branches.dedup();
    branches
        .into_iter()
        .filter_map(|branch| {
            let gained = after.xp.get(branch).copied().unwrap_or(0.0)
                - before.xp.get(branch).copied().unwrap_or(0.0);
            let wanted = expected.get(branch).copied().unwrap_or(0.0);
            ((gained - wanted).abs() > TOLERANCE)
                .then(|| format!("{who}: {branch} gained {gained:.3}, expected {wanted:.3}"))
        })
        .collect()
}

fn verdict(wrong: Vec<String>) -> Result<(), String> {
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("; "))
    }
}

/// ×2 for everything and ×1.5 more for the smith, with a general share of 0.5: a machine's ingots
/// pay three times their experience, and half of that goes to the general scale.
fn settings_multiply(game: &mut Game<'_>, tree: &Tree) -> Result<(), String> {
    let (branch, raw) = machine_gain(tree, "act:ingot", 4.0);
    let owner = glue::SELFTEST_PLAYER;
    let _settings = TestSettings::new(Settings {
        scale: XpScale {
            global: 2.0,
            branches: BTreeMap::from([(branch.clone(), 1.5)]),
            general_share: Some(0.5),
        },
        ..Settings::default()
    });
    let _online = Online::attach(tree, &[owner]);
    let before = profile(owner);
    report(game, &format!("smelt {} minecraft:iron_ingot 4", id(owner)))?;
    let after = profile(owner);
    let mut wrong = mismatches("owner", &before, &after, &gains([(branch, raw * 3.0)]));
    let general = after.general_xp - before.general_xp;
    if (general - raw * 3.0 * 0.5).abs() > TOLERANCE {
        wrong.push(format!(
            "general gained {general:.3}, expected {:.3}",
            raw * 1.5
        ));
    }
    verdict(wrong)
}

/// give, take, level, reset and show on a pretend owner, by UUID, as the console types them.
fn admin_commands(game: &mut Game<'_>, tree: &Tree) -> Result<(), String> {
    let owner = glue::SELFTEST_PLAYER;
    let uuid = id(owner);
    let _online = Online::attach(tree, &[owner]);
    let mut wrong = Vec::new();
    let mining = |p: &Profile| p.xp.get("mining").copied().unwrap_or(0.0);
    admin(game, &format!("pct admin give {uuid} mining 100"))?;
    if mining(&profile(owner)) != 100.0 {
        wrong.push(format!("give: mining {}", mining(&profile(owner))));
    }
    admin(game, &format!("pct admin take {uuid} mining 30"))?;
    if mining(&profile(owner)) != 70.0 {
        wrong.push(format!("take: mining {}", mining(&profile(owner))));
    }
    admin(game, &format!("pct admin give {uuid} minign 5"))?;
    if mining(&profile(owner)) != 70.0 || profile(owner).xp.contains_key("minign") {
        wrong.push("an unknown branch changed the profile".to_owned());
    }
    admin(game, &format!("pct admin level {uuid} mining 10"))?;
    let leveled = profile(owner);
    if tree.branch_level(&leveled, "mining") != 10 || mining(&leveled) != tree.xp_at_level(10) {
        wrong.push(format!("level: mining {}", mining(&leveled)));
    }
    let learned = glue::edit_profile(game, owner, |tree, p| {
        let node = tree
            .nodes
            .values()
            .find(|n| tree.can_allocate(p, n.id).is_ok())
            .ok_or("nothing to learn")?;
        tree.allocate(p, node.id).map_err(|r| format!("{r:?}"))
    });
    if let Err(why) = learned {
        wrong.push(format!("no node learned before the reset: {why}"));
    }
    admin(game, &format!("pct admin reset {uuid}"))?;
    let reset = profile(owner);
    if reset.ranks != tree.new_profile().ranks {
        wrong.push(format!("reset kept {} nodes", reset.ranks.len()));
    }
    if mining(&reset) != tree.xp_at_level(10) {
        wrong.push("reset changed the experience".to_owned());
    }
    let shown = admin(game, &format!("pct admin show {uuid}"))?;
    if shown.len() < tree.branches.len() + 1 || !shown[0].contains("в игре") {
        wrong.push(format!("show printed {shown:?}"));
    }
    verdict(wrong)
}

/// The summary a reader of version 1 knows stays as it was, and the new fields cover all sixteen
/// branches and the general scale.
fn summary_fields(game: &mut Game<'_>, tree: &Tree) -> Result<(), String> {
    let owner = glue::SELFTEST_PLAYER;
    let uuid = id(owner);
    let _online = Online::attach(tree, &[owner]);
    admin(game, &format!("pct admin level {uuid} mining 12"))?;
    admin(game, &format!("pct admin level {uuid} melee 3"))?;
    let summary = tree.combat_summary(&profile(owner));
    let json = serde_json::to_value(&summary).map_err(|e| e.to_string())?;
    let max = tree.curve.max_level;
    let keys = |field: &str| -> Vec<String> {
        json[field]
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    };
    let mut wrong = Vec::new();
    if json["v"] != 1 {
        wrong.push(format!("v is {}", json["v"]));
    }
    for field in ["levels", "ranks"] {
        if keys(field) != ["archery", "defense", "melee"] {
            wrong.push(format!("{field} has {:?}", keys(field)));
        }
    }
    if json["levels"]["melee"] != serde_json::json!([3, max]) {
        wrong.push(format!("levels.melee is {}", json["levels"]["melee"]));
    }
    let branches: Vec<String> = tree.branches.keys().cloned().collect();
    for field in ["all_levels", "all_ranks"] {
        if keys(field) != branches {
            wrong.push(format!("{field} has {:?}", keys(field)));
        }
    }
    if json["all_levels"]["mining"] != serde_json::json!([12, max]) {
        wrong.push(format!(
            "all_levels.mining is {}",
            json["all_levels"]["mining"]
        ));
    }
    if json["general"] != serde_json::json!([0, max]) {
        wrong.push(format!("general is {}", json["general"]));
    }
    verdict(wrong)
}

fn place(owner: u128, x: f64, dimension: &str, can_earn: bool) -> Nearby {
    Nearby {
        uuid: owner,
        dimension: dimension.to_owned(),
        position: [x, 64.0, 0.0],
        can_earn,
    }
}

/// The main owner and A are close, B is in the group but far, C is close but not in it. What the
/// main owner earns gives A a share; what A earns gives the main owner one, and no share is passed
/// on again.
fn group_share(game: &mut Game<'_>, tree: &Tree) -> Result<(), String> {
    let [main, a, b, c] = glue::SELFTEST_PLAYERS;
    let share = settings::group().share;
    let _online = Online::attach(tree, &glue::SELFTEST_PLAYERS);
    let places = vec![
        place(main, 0.0, DIMENSION, true),
        place(a, 10.0, DIMENSION, true),
        place(b, 100.0, DIMENSION, true),
        place(c, 5.0, DIMENSION, true),
    ];
    let _group = TestGroup::new(&[main, a, b], places);
    let before: Vec<Profile> = glue::SELFTEST_PLAYERS.map(profile).to_vec();
    report(game, &format!("smelt {} minecraft:iron_ingot 4", id(main)))?;
    report(game, &format!("fish {} minecraft:cod 1", id(a)))?;
    let (smith, ingots) = machine_gain(tree, "act:ingot", 4.0);
    let (fisher, fish) = machine_gain(tree, "act:fish", 1.0);
    let expected = [
        gains([(smith.clone(), ingots), (fisher.clone(), fish * share)]),
        gains([(smith, ingots * share), (fisher, fish)]),
        Gains::new(),
        Gains::new(),
    ];
    let names = ["main owner", "owner A", "owner B", "owner C"];
    let mut wrong = Vec::new();
    for (i, owner) in glue::SELFTEST_PLAYERS.iter().enumerate() {
        wrong.extend(mismatches(
            names[i],
            &before[i],
            &profile(*owner),
            &expected[i],
        ));
    }
    verdict(wrong)
}

/// Members who are creative or in another dimension get nothing, however close; a member who can
/// earn and is close does.
fn group_skips(game: &mut Game<'_>, tree: &Tree) -> Result<(), String> {
    let [main, a, b, c] = glue::SELFTEST_PLAYERS;
    let share = settings::group().share;
    let _online = Online::attach(tree, &glue::SELFTEST_PLAYERS);
    let places = vec![
        place(main, 0.0, DIMENSION, true),
        place(a, 1.0, DIMENSION, false),
        place(b, 1.0, OTHER_DIMENSION, true),
        place(c, 2.0, DIMENSION, true),
    ];
    let _group = TestGroup::new(&glue::SELFTEST_PLAYERS, places);
    let before: Vec<Profile> = glue::SELFTEST_PLAYERS.map(profile).to_vec();
    report(game, &format!("smelt {} minecraft:iron_ingot 2", id(main)))?;
    let (smith, ingots) = machine_gain(tree, "act:ingot", 2.0);
    let expected = [
        gains([(smith.clone(), ingots)]),
        Gains::new(),
        Gains::new(),
        gains([(smith, ingots * share)]),
    ];
    let names = [
        "main owner",
        "owner A (creative)",
        "owner B (nether)",
        "owner C",
    ];
    let mut wrong = Vec::new();
    for (i, owner) in glue::SELFTEST_PLAYERS.iter().enumerate() {
        wrong.extend(mismatches(
            names[i],
            &before[i],
            &profile(*owner),
            &expected[i],
        ));
    }
    verdict(wrong)
}
