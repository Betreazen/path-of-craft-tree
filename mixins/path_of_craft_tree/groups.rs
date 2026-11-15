// Group play on the server: groups live in memory (group_rules.rs) and end with the server or when
// their players leave. A member near an earner gets a share of the earner's main experience as
// extra, to their own same branch (glue::grant). Only real online players take part; the
// self-test puts its pretend owners in a group of their own with made-up positions.
use crate::game::{
    Game, pct_glue as glue,
    pct_group_rules::{self as rules, Groups, Nearby, Refusal},
    pct_rules::Tree,
    pct_settings as settings,
};
use horizon_client::text::Text;
use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

/// How often the list of online players, their places and names is refreshed. Inside a
/// player's turn the server's list is out of reach, so shares and commands read this copy.
const ROSTER_EVERY_TICKS: u32 = 10;
const SURVIVAL: i32 = 0;
const ADVENTURE: i32 = 2;

#[derive(Default)]
struct State {
    groups: Groups,
    roster: Vec<Nearby>,
    names: BTreeMap<u128, String>,
    /// Lines for players who are not the one acting, sent on the next server tick.
    outbox: Vec<(u128, String)>,
    /// The self-test's pretend owners with their made-up places, while it runs.
    test: Vec<Nearby>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Server tick: refreshes the roster and sends what waits in the outbox.
pub fn on_server_tick(game: &mut Game<'_>, ticks: u32) {
    if ticks % ROSTER_EVERY_TICKS == 0 {
        refresh_roster(game);
    }
    let outbox = std::mem::take(&mut state().outbox);
    for (player, line) in outbox {
        let lang = crate::game::pct_texts::player_lang(game, player);
        let line = crate::game::pct_texts::tr_in(lang, &line).into_owned();
        game.native_tell(player, Text::literal(line));
    }
}

fn refresh_roster(game: &Game<'_>) {
    let players = game.native_server_players();
    let roster = players
        .iter()
        .filter(|p| !glue::is_test_player(p.uuid))
        .map(|p| Nearby {
            uuid: p.uuid,
            dimension: p.dimension.clone(),
            position: p.position,
            can_earn: matches!(p.game_mode, SURVIVAL | ADVENTURE),
        })
        .collect();
    let names = players.into_iter().map(|p| (p.uuid, p.name)).collect();
    let mut state = state();
    state.roster = roster;
    state.names = names;
}

/// An online player by name (any case) or UUID: their UUID and name. The server's own list where
/// it is in reach, else the roster.
pub fn find_online(game: &Game<'_>, word: &str) -> Option<(u128, String)> {
    let live: Vec<(u128, String)> = game
        .native_server_players()
        .into_iter()
        .map(|p| (p.uuid, p.name))
        .collect();
    let known = if live.is_empty() {
        state().names.clone().into_iter().collect()
    } else {
        live
    };
    let uuid = crate::game::pct_command_rules::parse_uuid(word);
    known
        .into_iter()
        .find(|(id, name)| Some(*id) == uuid || name.eq_ignore_ascii_case(word))
}

/// The name an online player goes by, as the roster last saw it.
pub fn known_name(player: u128) -> Option<String> {
    state().names.get(&player).cloned()
}

/// The name an online player goes by, else their UUID.
pub fn name_of(player: u128) -> String {
    known_name(player).unwrap_or_else(|| format!("{player:032x}"))
}

/// Tells `player` a line on the next server tick.
pub fn tell_later(player: u128, line: String) {
    state().outbox.push((player, line));
}

pub fn invite(from: u128, to: u128) -> Result<(), Refusal> {
    let max = settings::group().max_size;
    state().groups.invite(from, to, max, Instant::now())
}

pub fn accept(player: u128) -> Result<Vec<u128>, Refusal> {
    let max = settings::group().max_size;
    state().groups.accept(player, max, Instant::now())
}

pub fn leave(player: u128) -> Result<Vec<u128>, Refusal> {
    state().groups.leave(player)
}

/// The player's group, themselves included; empty when they are in none.
pub fn members(player: u128) -> Vec<u128> {
    state().groups.members(player).to_vec()
}

/// Who invited the player, while the invitation holds.
pub fn invited_by(player: u128) -> Option<u128> {
    state().groups.invited_by(player, Instant::now())
}

/// Pays the earner's group members nearby their share of `gained` in `branch`, once for this
/// earning: a share is credited without passing a share on.
pub fn share(game: &mut Game<'_>, tree: &Tree, earner: u128, branch: &str, gained: f64) {
    let recipients = {
        let state = state();
        if state.groups.is_empty() {
            return;
        }
        let members = state.groups.members(earner);
        if members.is_empty() {
            return;
        }
        let radius = settings::group().radius;
        if state.test.is_empty() {
            rules::share_recipients(earner, members, &state.roster, radius)
        } else {
            let roster: Vec<Nearby> = state.roster.iter().chain(&state.test).cloned().collect();
            rules::share_recipients(earner, members, &roster, radius)
        }
    };
    let amount = rules::share_amount(gained, settings::group().share);
    if amount <= 0.0 {
        return;
    }
    for member in recipients {
        glue::credit_share(game, tree, member, branch, amount);
    }
}

pub fn forget_player(player: u128) {
    let mut state = state();
    let mates = state.groups.members(player).to_vec();
    state.groups.forget(player);
    let name = state
        .names
        .get(&player)
        .cloned()
        .unwrap_or_else(|| format!("{player:032x}"));
    for mate in mates.into_iter().filter(|mate| *mate != player) {
        state
            .outbox
            .push((mate, format!("{name} вышел из игры и покинул группу")));
    }
}

pub fn forget_world() {
    *state() = State::default();
}

/// Self-test only: pretend owners `members` form a group, standing where `places` says, until it
/// is dropped. Real players are never put in it.
pub struct TestGroup {
    members: Vec<u128>,
}

impl TestGroup {
    pub fn new(members: &[u128], places: Vec<Nearby>) -> Self {
        let members: Vec<u128> = members
            .iter()
            .copied()
            .filter(|m| glue::is_test_player(*m))
            .collect();
        let mut state = state();
        let now = Instant::now();
        let max = members.len().max(2);
        if let Some((first, rest)) = members.split_first() {
            for member in rest {
                let _ = state.groups.invite(*first, *member, max, now);
                let _ = state.groups.accept(*member, max, now);
            }
        }
        state.test = places
            .into_iter()
            .filter(|p| glue::is_test_player(p.uuid))
            .collect();
        TestGroup { members }
    }
}

impl Drop for TestGroup {
    fn drop(&mut self) {
        let mut state = state();
        for member in &self.members {
            state.groups.forget(*member);
        }
        state.test.clear();
    }
}
