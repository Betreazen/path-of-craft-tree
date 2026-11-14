// Connects the rules to the game. The server owns the state: the client only asks.
use crate::game::{
    Game, native_hooks::NativeScreenView, pct_placed as placed, pct_rules as rules,
    pct_view as view,
};
use horizon_client::text::Text;
use horizon_world::nbt::{JavaString, Tag};
use rules::{Assembler, Effects, Profile, Tree};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant, SystemTime},
};
use view::Camera;

/// Dev builds (dev.ps1 sets `PCT_EXTERNAL_TREE` when compiling) read the tree from a config file
/// and reload it when it changes. Performance and release builds use only the tree compiled in.
const EXTERNAL_TREE: bool = option_env!("PCT_EXTERNAL_TREE").is_some();
const BUILT_IN_TREE: &[u8] = horizon_client::native_data!("path_of_craft_tree", "tree.json");

pub const ID: &str = "path_of_craft_tree";
const HELLO: &str = "path_of_craft_tree:hello";
const ALLOCATE: &str = "path_of_craft_tree:allocate";
const TOGGLE: &str = "path_of_craft_tree:toggle";
const RESPEC: &str = "path_of_craft_tree:respec";
const TREE: &str = "path_of_craft_tree:tree";
const STATE: &str = "path_of_craft_tree:state";
const CONFIG_FILE: &str = "path_of_craft_tree.json";

const RELOAD_EVERY_TICKS: u32 = 40;
const SAVE_EVERY_TICKS: u32 = 1200;
/// How long a joining client has to prove it runs the mod before it is disconnected.
const HELLO_TIMEOUT: Duration = Duration::from_secs(15);
const FEEDBACK_EVERY: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct Loaded {
    pub tree: Arc<Tree>,
    bytes: Arc<Vec<u8>>,
    modified: Option<SystemTime>,
}

/// A connected player's progress, kept in memory and saved periodically.
pub struct Entry {
    pub profile: Profile,
    pub effects: Arc<Effects>,
    dirty: bool,
    feedback_at: Instant,
    unreported_xp: f64,
    unreported_branch: String,
    /// The combat summary other mods read may have changed: written on the next server tick.
    summary_due: bool,
    /// The summary last written to the player's persistent data, to skip writing the same again.
    summary_written: Option<String>,
}

#[derive(Default)]
pub struct Server {
    loaded: Option<Loaded>,
    rejected: Option<SystemTime>,
    pub players: BTreeMap<u128, Entry>,
    /// Players who joined and have not answered the hello yet, and when they joined.
    awaiting_hello: BTreeMap<u128, Instant>,
    server_ticks: u32,
    pub placed: placed::Placed,
    /// Positions of blocks a player placed and just broke, until the loot hook has seen them.
    pub just_broken: HashSet<(String, placed::Position)>,
}

#[derive(Default)]
pub struct Client {
    pub tree: Option<Arc<Tree>>,
    pub profile: Profile,
    pub effects: Arc<Effects>,
    assembler: Assembler,
    pub problem: Option<String>,
    pub camera: Camera,
    pub press: Option<[f32; 2]>,
    pub travelled: f32,
    pub requests: Vec<u16>,
    pub toggles: Vec<u16>,
    pub respec_requested: bool,
    pub respec_armed: bool,
    owe_hello: bool,
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(Mutex::default);
static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(Mutex::default);

pub fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn client() -> MutexGuard<'static, Client> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The protocol this client announces; the test override simulates an outdated client.
fn client_protocol() -> u16 {
    static OVERRIDE: LazyLock<Option<u16>> = LazyLock::new(|| {
        std::env::var("PATH_OF_CRAFT_TREE_TEST_PROTOCOL")
            .ok()
            .and_then(|value| value.parse().ok())
    });
    OVERRIDE
        .filter(|_| EXTERNAL_TREE)
        .unwrap_or(rules::PROTOCOL)
}

// The tree file.

fn tree_path() -> Result<PathBuf, String> {
    Ok(Game::native_config_dir()?.join(CONFIG_FILE))
}

fn modified_time() -> Option<SystemTime> {
    if !EXTERNAL_TREE {
        return None;
    }
    std::fs::metadata(tree_path().ok()?).ok()?.modified().ok()
}

fn read_tree_file() -> Result<Vec<u8>, String> {
    let path = tree_path()?;
    if !path.exists() {
        std::fs::write(&path, BUILT_IN_TREE).map_err(|e| e.to_string())?;
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

fn built_in() -> Option<Loaded> {
    match Tree::parse(BUILT_IN_TREE) {
        Ok(tree) => Some(Loaded {
            tree: Arc::new(tree),
            bytes: Arc::new(BUILT_IN_TREE.to_vec()),
            modified: None,
        }),
        Err(error) => {
            log::error!("path_of_craft_tree: the built-in tree is invalid: {error}");
            None
        }
    }
}

fn parse_external(modified: Option<SystemTime>) -> Result<Loaded, String> {
    let bytes = read_tree_file()?;
    let tree = Tree::parse(&bytes)?;
    Ok(Loaded {
        tree: Arc::new(tree),
        bytes: Arc::new(bytes),
        modified: modified_time().or(modified),
    })
}

/// Reads the tree file when it changed; true when a new tree took effect. A broken file is
/// reported once and the previous tree stays in use.
fn reload_if_changed() -> bool {
    if !EXTERNAL_TREE {
        let mut state = server();
        if state.loaded.is_none() {
            state.loaded = built_in();
            return state.loaded.is_some();
        }
        return false;
    }
    let modified = modified_time();
    {
        let state = server();
        let current = state
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.modified == modified);
        if current || (state.rejected == modified && state.loaded.is_some()) {
            return false;
        }
    }
    match parse_external(modified) {
        Ok(loaded) => {
            log::info!(
                "path_of_craft_tree: tree loaded, {} nodes",
                loaded.tree.nodes.len()
            );
            let mut state = server();
            state.loaded = Some(loaded);
            state.rejected = None;
            true
        }
        Err(error) => {
            log::warn!("path_of_craft_tree: tree file rejected, keeping the previous one: {error}");
            let mut state = server();
            state.rejected = modified;
            if state.loaded.is_none() {
                state.loaded = built_in();
            }
            false
        }
    }
}

pub fn current_tree() -> Option<Loaded> {
    if server().loaded.is_none() {
        reload_if_changed();
    }
    server().loaded.clone()
}

// Saved progress.

pub fn mod_dir(game: &Game<'_>) -> Option<PathBuf> {
    Some(game.native_world_dir()?.join(ID))
}

fn state_file(game: &Game<'_>, player: u128) -> Option<PathBuf> {
    Some(mod_dir(game)?.join(format!("{player:032x}.json")))
}

/// Replaces `path` in one step, so a crash mid-write cannot leave half a file behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}

/// The saved profile fitted to `tree`. A file that cannot be read is kept aside instead of being
/// overwritten; one saved under the old node numbering keeps only its experience.
fn read_saved_profile(path: &Path, tree: &Tree) -> Option<Profile> {
    let bytes = std::fs::read(path).ok()?;
    match tree.load_saved(&bytes) {
        Ok((profile, dropped)) => {
            if dropped {
                log::info!(
                    "path_of_craft_tree: {} was saved for the old tree: nodes cleared, experience kept",
                    path.display()
                );
            }
            Some(profile)
        }
        Err(error) => {
            log::error!("path_of_craft_tree: {} is damaged: {error}", path.display());
            if let Err(error) = std::fs::rename(path, path.with_extension("bad")) {
                log::error!("path_of_craft_tree: damaged file not kept aside: {error}");
            }
            None
        }
    }
}

fn load_profile(game: &Game<'_>, tree: &Tree, player: u128) -> Profile {
    state_file(game, player)
        .and_then(|path| read_saved_profile(&path, tree))
        .unwrap_or_else(|| tree.new_profile())
}

fn write_profile(game: &Game<'_>, player: u128, profile: &Profile) -> Result<(), String> {
    let path = state_file(game, player).ok_or("no world is open")?;
    let dir = path.parent().ok_or("the state file has no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_vec(profile).map_err(|e| e.to_string())?;
    write_atomic(&path, &json)
}

fn apply_attributes(game: &mut Game<'_>, tree: &Tree, player: u128, profile: &Profile) {
    for (attribute, value) in tree.attributes(&profile.ranks) {
        if !game.native_set_attribute_base(player, &attribute, value) {
            log::warn!("path_of_craft_tree: {attribute} not set for {player:032x}");
        }
    }
}

fn save_player(game: &Game<'_>, player: u128) {
    if player == SELFTEST_PLAYER {
        return;
    }
    let snapshot = {
        let mut state = server();
        state
            .players
            .get_mut(&player)
            .filter(|entry| entry.dirty)
            .map(|entry| {
                entry.dirty = false;
                entry.profile.clone()
            })
    };
    if let Some(profile) = snapshot {
        if let Err(error) = write_profile(game, player, &profile) {
            log::warn!("path_of_craft_tree: progress not saved: {error}");
            if let Some(entry) = server().players.get_mut(&player) {
                entry.dirty = true;
            }
        }
    }
}

fn save_placed(game: &Game<'_>) {
    let Some(dir) = mod_dir(game).map(|dir| dir.join("placed")) else {
        return;
    };
    let regions = server().placed.take_dirty();
    if regions.is_empty() {
        return;
    }
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::warn!("path_of_craft_tree: placed blocks not saved: {error}");
        return;
    }
    for (key, bytes) in regions {
        if let Err(error) = write_atomic(&dir.join(key.file_name()), &bytes) {
            log::warn!("path_of_craft_tree: placed blocks not saved: {error}");
        }
    }
}

fn save_everything(game: &Game<'_>) {
    let players: Vec<u128> = server().players.keys().copied().collect();
    for player in players {
        save_player(game, player);
        crate::game::pct_actions::save_chunks(game, player);
    }
    save_placed(game);
}

/// Writes `bytes` to `path` atomically, creating its folder first.
pub fn write_in_folder(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    write_atomic(path, bytes)
}

/// An orderly server stop: nothing earned since the last periodic save is lost.
pub fn on_server_stopping(game: &mut Game<'_>) {
    save_everything(game);
}

/// The folder a region's placed blocks are read from.
pub fn placed_dir(game: &Game<'_>) -> Option<PathBuf> {
    Some(mod_dir(game)?.join("placed"))
}

// Server half.

fn send(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if let Err(error) = game.native_net_send_to_player(player, channel, data) {
        log::warn!("path_of_craft_tree: {channel} not sent: {error}");
    }
}

fn send_state(game: &mut Game<'_>, player: u128, profile: &Profile) {
    send_encoded_state(game, player, serde_json::to_vec(&profile.for_client()));
}

fn send_encoded_state(game: &mut Game<'_>, player: u128, encoded: serde_json::Result<Vec<u8>>) {
    match encoded {
        Ok(json) => {
            crate::game::pct_perf::add(crate::game::pct_perf::Probe::StateBytes, json.len() as u64);
            send(game, player, STATE, &json);
        }
        Err(error) => log::warn!("path_of_craft_tree: state not encoded: {error}"),
    }
}

fn publish(game: &mut Game<'_>, player: u128, loaded: &Loaded, profile: &Profile) {
    for part in rules::split_parts(&loaded.bytes) {
        send(game, player, TREE, &part);
    }
    send_state(game, player, profile);
}

fn kick(game: &mut Game<'_>, player: u128, reason: String) {
    log::warn!("path_of_craft_tree: disconnecting {player:032x}: {reason}");
    if !game.native_kick(player, &Text::literal(reason)) {
        log::warn!("path_of_craft_tree: {player:032x} could not be disconnected");
    }
}

pub fn on_login(game: &mut Game<'_>, player: u128) {
    server().awaiting_hello.insert(player, Instant::now());
    send(game, player, HELLO, &rules::PROTOCOL.to_le_bytes());
}

pub fn on_logout(game: &mut Game<'_>, player: u128) {
    save_player(game, player);
    save_placed(game);
    crate::game::pct_actions::save_chunks(game, player);
    crate::game::pct_actions::forget_player(player);
    let mut state = server();
    state.players.remove(&player);
    state.awaiting_hello.remove(&player);
}

/// Disconnects clients that never answered: they do not have the mod.
fn kick_silent_clients(game: &mut Game<'_>) {
    let overdue: Vec<u128> = {
        let mut state = server();
        let overdue = state
            .awaiting_hello
            .iter()
            .filter(|(_, joined)| joined.elapsed() >= HELLO_TIMEOUT)
            .map(|(player, _)| *player)
            .collect::<Vec<_>>();
        for player in &overdue {
            state.awaiting_hello.remove(player);
        }
        overdue
    };
    for player in overdue {
        let reason = format!(
            "Для игры на этом сервере нужен мод Path-of-craft-tree (протокол {})",
            rules::PROTOCOL
        );
        kick(game, player, reason);
    }
}

fn on_hello(game: &mut Game<'_>, player: u128, data: &[u8]) {
    server().awaiting_hello.remove(&player);
    let protocol = rules::decode_u16(data);
    if protocol != Some(rules::PROTOCOL) {
        server().players.remove(&player);
        let reason = format!(
            "Нужна другая версия мода Path-of-craft-tree: на сервере протокол {}, у вас {}",
            rules::PROTOCOL,
            protocol.map_or_else(|| "?".to_owned(), |p| p.to_string())
        );
        kick(game, player, reason);
        return;
    }
    // A client only greets once per join: a repeat would reload the file over unsaved progress.
    if server().players.contains_key(&player) {
        return;
    }
    let Some(loaded) = current_tree() else {
        return;
    };
    let profile = load_profile(game, &loaded.tree, player);
    let entry = Entry {
        effects: Arc::new(loaded.tree.effects(&profile)),
        profile: profile.clone(),
        // Written back at the next save, so a file fitted to a changed tree is stored that way.
        dirty: true,
        feedback_at: Instant::now(),
        unreported_xp: 0.0,
        unreported_branch: String::new(),
        summary_due: true,
        summary_written: None,
    };
    server().players.insert(player, entry);
    log::info!(
        "path_of_craft_tree: {player:032x} matched protocol {}",
        rules::PROTOCOL
    );
    apply_attributes(game, &loaded.tree, player, &profile);
    publish(game, player, &loaded, &profile);
}

fn on_allocate(game: &mut Game<'_>, player: u128, data: &[u8]) {
    let (Some(id), Some(loaded)) = (rules::decode_u16(data), current_tree()) else {
        log::warn!("path_of_craft_tree: malformed request from {player:032x}");
        return;
    };
    let (result, changed) = {
        let mut state = server();
        let Some(entry) = state.players.get_mut(&player) else {
            log::warn!("path_of_craft_tree: {player:032x} asked without a matching protocol");
            return;
        };
        let changed = match loaded.tree.allocate(&entry.profile, id) {
            Ok(next) => {
                log::info!(
                    "path_of_craft_tree: {player:032x} allocated node {id}, rank {}",
                    next.rank(id)
                );
                entry.effects = Arc::new(loaded.tree.effects(&next));
                entry.summary_due = true;
                entry.profile = next;
                entry.dirty = true;
                true
            }
            Err(refusal) => {
                log::info!("path_of_craft_tree: {player:032x} refused {id}: {refusal:?}");
                false
            }
        };
        (entry.profile.clone(), changed)
    };
    if changed {
        save_player(game, player);
        apply_attributes(game, &loaded.tree, player, &result);
    }
    send_state(game, player, &result);
}

/// Stores a changed profile, reapplies what depends on it and tells the player.
fn replace_profile(game: &mut Game<'_>, player: u128, loaded: &Loaded, next: Profile) {
    if let Some(entry) = server().players.get_mut(&player) {
        entry.effects = Arc::new(loaded.tree.effects(&next));
        entry.summary_due = true;
        entry.profile = next.clone();
        entry.dirty = true;
    }
    save_player(game, player);
    apply_attributes(game, &loaded.tree, player, &next);
    send_state(game, player, &next);
}

fn current_profile(player: u128) -> Option<Profile> {
    server()
        .players
        .get(&player)
        .map(|entry| entry.profile.clone())
}

fn on_toggle(game: &mut Game<'_>, player: u128, data: &[u8]) {
    let (Some(id), Some(loaded), Some(profile)) = (
        rules::decode_u16(data),
        current_tree(),
        current_profile(player),
    ) else {
        return;
    };
    if let Some(next) = loaded.tree.toggle(&profile, id) {
        replace_profile(game, player, &loaded, next);
    }
}

fn on_respec(game: &mut Game<'_>, player: u128) {
    let (Some(loaded), Some(profile)) = (current_tree(), current_profile(player)) else {
        return;
    };
    let level = game
        .native_server_players()
        .into_iter()
        .find(|online| online.uuid == player)
        .map_or(0, |online| online.xp_level);
    if level < rules::RESPEC_LEVELS {
        game.native_action_bar(
            player,
            Text::literal("Для сброса дерева не хватает уровней опыта".to_owned()),
        );
        return;
    }
    if !game.native_give_levels(player, -rules::RESPEC_LEVELS) {
        log::warn!("path_of_craft_tree: levels not taken from {player:032x}");
        return;
    }
    log::info!("path_of_craft_tree: {player:032x} reset the tree");
    replace_profile(game, player, &loaded, loaded.tree.respec(&profile));
}

pub fn on_client_message(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    match channel {
        HELLO => on_hello(game, player, data),
        ALLOCATE => on_allocate(game, player, data),
        TOGGLE => on_toggle(game, player, data),
        RESPEC => on_respec(game, player),
        _ => {}
    }
}

/// Records a one-time reward; true the first time this key is seen for the player.
pub fn mark_seen(player: u128, key: String) -> bool {
    let mut state = server();
    let Some(entry) = state.players.get_mut(&player) else {
        return false;
    };
    let first = entry.profile.seen.insert(key);
    entry.dirty |= first;
    first
}

/// The player placed one more `block` of sand or gravel (mining.rs): kept in their profile.
pub fn count_placed_falling(player: u128, block: &str) {
    if let Some(entry) = server().players.get_mut(&player) {
        *entry
            .profile
            .placed_falling
            .entry(block.to_owned())
            .or_default() += 1;
        entry.dirty = true;
    }
}

/// Whether a broken `block` of sand or gravel is paid for by one the player placed earlier, which
/// it then uses up.
pub fn take_placed_falling(player: u128, block: &str) -> bool {
    let mut state = server();
    let Some(entry) = state.players.get_mut(&player) else {
        return false;
    };
    let Some(count) = entry.profile.placed_falling.get_mut(block) else {
        return false;
    };
    *count -= 1;
    if *count == 0 {
        entry.profile.placed_falling.remove(block);
    }
    entry.dirty = true;
    true
}

/// The pretend owner the self-test (selftest.rs) credits. It is online only inside one console
/// command: nothing is sent to it and its profile is never saved.
pub const SELFTEST_PLAYER: u128 = 0x7e57_7e57_0000_4000_8000_0000_0000_0001;

/// Self-test only: puts the pretend owner online with a fresh profile, replacing any earlier one.
pub fn attach_test_player(tree: &Tree) {
    let profile = tree.new_profile();
    let entry = Entry {
        effects: Arc::new(tree.effects(&profile)),
        profile,
        dirty: false,
        feedback_at: Instant::now(),
        unreported_xp: 0.0,
        unreported_branch: String::new(),
        summary_due: true,
        summary_written: None,
    };
    server().players.insert(SELFTEST_PLAYER, entry);
}

/// Self-test only: the pretend owner's experience by branch.
pub fn test_player_xp() -> BTreeMap<String, f64> {
    server()
        .players
        .get(&SELFTEST_PLAYER)
        .map(|entry| entry.profile.xp.clone())
        .unwrap_or_default()
}

/// Self-test only: takes the pretend owner offline again, without saving anything.
pub fn detach_test_player() {
    server().players.remove(&SELFTEST_PLAYER);
}

/// Grants experience and tells the player about it, at most a few times a second.
pub fn grant(
    game: &mut Game<'_>,
    tree: &Tree,
    player: u128,
    branch: &str,
    change: impl FnOnce(&Tree, &mut Profile),
    gained: f64,
) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Grant);
    let report = {
        let mut state = server();
        let Some(entry) = state.players.get_mut(&player) else {
            return;
        };
        // Only the experience table is kept from before; the profile changes in place.
        let before = entry.profile.xp.clone();
        change(tree, &mut entry.profile);
        let level = tree.branch_level(&entry.profile, branch);
        let leveled = level
            > tree
                .level_progress(before.get(branch).copied().unwrap_or(0.0))
                .0;
        // The level bonus of every branch is part of the effects, so a new level of any branch refreshes them.
        if tree.levels_changed(&before, &entry.profile.xp) {
            entry.effects = Arc::new(tree.effects(&entry.profile));
            entry.summary_due = true;
        }
        entry.dirty = true;
        if entry.unreported_branch != branch {
            entry.unreported_branch = branch.to_owned();
            entry.unreported_xp = 0.0;
        }
        entry.unreported_xp += gained;
        if entry.feedback_at.elapsed() >= FEEDBACK_EVERY || leveled {
            entry.feedback_at = Instant::now();
            // Encoded here rather than copied out: the message is all that leaves the lock.
            Some((
                std::mem::take(&mut entry.unreported_xp),
                serde_json::to_vec(&entry.profile.for_client()),
                leveled,
                level,
            ))
        } else {
            None
        }
    };
    if player == SELFTEST_PLAYER {
        return;
    }
    if let Some((xp, encoded, leveled, level)) = report {
        let name = tree
            .branches
            .get(branch)
            .map_or(branch, |b| b.name.as_str());
        let line = format!("+{xp:.1} опыта · {name} {level}");
        game.native_action_bar(player, Text::literal(line));
        send_encoded_state(game, player, encoded);
        if leveled {
            save_player(game, player);
        }
    }
}

fn push_tree_to_players(game: &mut Game<'_>, loaded: &Loaded) {
    let players: Vec<(u128, Profile)> = {
        let mut state = server();
        state
            .players
            .iter_mut()
            .map(|(player, entry)| {
                let next = loaded.tree.sanitize(&entry.profile);
                entry.effects = Arc::new(loaded.tree.effects(&next));
                entry.summary_due = true;
                entry.dirty |= next != entry.profile;
                entry.profile = next.clone();
                (*player, next)
            })
            .collect()
    };
    for (player, profile) in players {
        save_player(game, player);
        apply_attributes(game, &loaded.tree, player, &profile);
        publish(game, player, loaded, &profile);
    }
}

/// The key of the combat summary in a player's persistent data, which mods share; SoulsCraft
/// reads it to scale its enemies and never writes it.
pub const SUMMARY_KEY: &str = "pct:summary";

/// Writes the combat summary (`Tree::combat_summary`, JSON) of each player whose tree changed into
/// their persistent data, beside other mods' keys. Done on the server tick: inside a player's turn
/// only that player can be reached. A player not reached keeps it due for the next tick.
fn write_summaries(game: &mut Game<'_>) {
    let Some(loaded) = current_tree() else {
        return;
    };
    let due: Vec<(u128, String)> = {
        let mut state = server();
        state
            .players
            .iter_mut()
            .filter(|(player, entry)| entry.summary_due && **player != SELFTEST_PLAYER)
            .filter_map(|(player, entry)| {
                let summary = loaded.tree.combat_summary(&entry.profile);
                let json = serde_json::to_string(&summary).ok()?;
                if entry.summary_written.as_ref() == Some(&json) {
                    entry.summary_due = false;
                    return None;
                }
                Some((*player, json))
            })
            .collect()
    };
    for (player, json) in due {
        if !put_summary(game, player, &json) {
            continue;
        }
        if let Some(entry) = server().players.get_mut(&player) {
            entry.summary_due = false;
            entry.summary_written = Some(json);
        }
    }
}

/// Puts `json` under the summary key of the player's persistent data, keeping the other keys.
fn put_summary(game: &mut Game<'_>, player: u128, json: &str) -> bool {
    let Some(mut data) = game.native_player_persistent_data(player) else {
        return false;
    };
    let Tag::Compound(map) = &mut data else {
        return false;
    };
    map.insert(
        JavaString::from(SUMMARY_KEY),
        Tag::String(JavaString::from(json)),
    );
    game.native_set_player_persistent_data(player, data)
}

pub fn on_server_tick(game: &mut Game<'_>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::ServerTick);
    crate::game::pct_perf::report_if_due("server");
    let ticks = {
        let mut state = server();
        state.server_ticks = state.server_ticks.wrapping_add(1);
        state.just_broken.clear();
        state.server_ticks
    };
    crate::game::pct_actions::on_server_tick(game);
    crate::game::pct_selftest::on_server_tick(game);
    crate::game::pct_mining::apply_piston_moves(game);
    write_summaries(game);
    if ticks % SAVE_EVERY_TICKS == 0 {
        save_everything(game);
    }
    if ticks % RELOAD_EVERY_TICKS != 0 {
        return;
    }
    kick_silent_clients(game);
    if !reload_if_changed() {
        return;
    }
    if let Some(loaded) = current_tree() {
        push_tree_to_players(game, &loaded);
    }
}

// Client half. A single-player world talks to its built-in server over the same messages.

pub fn on_server_message(channel: &str, data: &[u8]) {
    let mut view = client();
    match channel {
        HELLO => {
            let server_protocol = rules::decode_u16(data);
            view.tree = None;
            view.profile = Profile::default();
            view.effects = Arc::default();
            view.owe_hello = true;
            view.problem = (server_protocol != Some(client_protocol())).then(|| {
                format!(
                    "Версия мода не совпадает: у вас {}, на сервере {}",
                    client_protocol(),
                    server_protocol.map_or_else(|| "?".to_owned(), |p| p.to_string())
                )
            });
        }
        TREE => {
            if let Some(bytes) = view.assembler.push(data) {
                match Tree::parse(&bytes) {
                    Ok(tree) => {
                        view.effects = Arc::new(tree.effects(&view.profile));
                        view.tree = Some(Arc::new(tree));
                    }
                    Err(error) => view.problem = Some(format!("Дерево не прочитано: {error}")),
                }
            }
        }
        STATE => {
            if let Ok(profile) = serde_json::from_slice::<Profile>(data) {
                if let Some(tree) = &view.tree {
                    view.effects = Arc::new(tree.effects(&profile));
                }
                view.profile = profile;
            }
        }
        _ => {}
    }
}

fn open(game: &mut Game<'_>) {
    if game.native_playing() {
        game.native_mod_screen_open(ID);
    }
}

pub fn on_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if pressed && crate::game::pct_keys::is(game, key, crate::game::pct_keys::Key::Tree) {
        open(game);
    }
}

pub fn on_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if command.trim() == "tree" {
        *ran = true;
        open(game);
    }
}

pub fn on_frame(game: &mut Game<'_>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::ClientFrame);
    crate::game::pct_perf::report_if_due("client");
    if client().owe_hello {
        let hello = client_protocol().to_le_bytes();
        if game.native_net_send_to_server(HELLO, &hello).is_ok() {
            client().owe_hello = false;
        }
    }
    let requests = std::mem::take(&mut client().requests);
    for id in requests {
        if let Err(error) = game.native_net_send_to_server(ALLOCATE, &id.to_le_bytes()) {
            log::warn!("path_of_craft_tree: request not sent: {error}");
        }
    }
    let toggles = std::mem::take(&mut client().toggles);
    for id in toggles {
        if let Err(error) = game.native_net_send_to_server(TOGGLE, &id.to_le_bytes()) {
            log::warn!("path_of_craft_tree: toggle not sent: {error}");
        }
    }
    if std::mem::take(&mut client().respec_requested) {
        if let Err(error) = game.native_net_send_to_server(RESPEC, &[]) {
            log::warn!("path_of_craft_tree: reset not sent: {error}");
        }
    }
    crate::game::pct_art::on_frame(game);
}
