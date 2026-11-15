// The tree window's tools around the tree, and the keys they take: the search box with the
// filter of what can be taken now, the planner with its share strings, the statistics page and
// the switch of the experience bar on the screen. ui.rs draws the tree and asks here first;
// window_draw.rs draws these tools. The state lives here, apart from the shared client state:
// this module never holds its own lock while it takes glue's, nor the other way round.
use crate::game::pct_window_draw::{Panel, PlanView};
use crate::game::{
    Game,
    native_hooks::NativeScreenView,
    pct_art as art, pct_find as find, pct_gains as gains,
    pct_glue::{ID, client},
    pct_keys as keys, pct_look as look, pct_plan as plan, pct_rules as rules,
    pct_session as session, pct_ui as ui, pct_view as view,
};
use find::{Found, Index};
use horizon_client::clipboard::Clipboard as _;
use look::{Look, Shown};
use plan::Plan;
use rules::{Profile, Tree};
use session::Session;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub const WHITE: u32 = 0xFFFFFF;
pub const GREY: u32 = 0xAAAAAA;
pub const RED: u32 = 0xFF5555;
pub const GREEN: u32 = 0x55FF55;
pub const AQUA: u32 = 0x55FFFF;
/// How long a message under the tools stays.
const MESSAGE_FOR: Duration = Duration::from_secs(7);
/// Plan ranks sent per frame while a plan is applied: the game takes 32 messages a tick, and
/// every answer is a state message.
const APPLY_PER_FRAME: usize = 4;
const SETTINGS_FILE: &str = "path_of_craft_tree_client.txt";
const BAR_SETTING: &str = "xp_bar=";

const ESCAPE: &str = "key.keyboard.escape";
const ENTER: [&str; 2] = ["key.keyboard.enter", "key.keyboard.keypad.enter"];
const BACKSPACE: &str = "key.keyboard.backspace";
const UP: &str = "key.keyboard.up";
const DOWN: &str = "key.keyboard.down";
const PASTE: &str = "key.keyboard.v";
const COPY: &str = "key.keyboard.c";
/// Keys that type a character: their presses are kept from the screen (the inventory key would
/// close the window), and their characters come to `on_char`.
const TYPING: [&str; 14] = [
    "space",
    "minus",
    "equal",
    "left.bracket",
    "right.bracket",
    "backslash",
    "semicolon",
    "apostrophe",
    "grave.accent",
    "comma",
    "period",
    "slash",
    "world.1",
    "international1",
];
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;
const ALT: u32 = 4;

/// The tools' buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Available,
    Planner,
    Stats,
    Apply,
    Clear,
    Copy,
    Bar,
    Close,
}

#[derive(Default)]
struct FoundFor {
    tree: Option<Arc<Tree>>,
    profile: Option<Arc<Profile>>,
    query: String,
    only_available: bool,
    found: Arc<Found>,
}

pub struct Window {
    pub query: String,
    pub only_available: bool,
    pub planning: bool,
    pub stats: bool,
    /// The match the camera went to last, as an index into the matches.
    pub current: Option<usize>,
    plan: Plan,
    /// Grows with every change to the plan.
    plan_revision: u64,
    plan_view: Option<(Arc<Tree>, Arc<Profile>, u64, Arc<PlanView>)>,
    pub message: Option<(String, u32, Instant)>,
    applying: VecDeque<u16>,
    paste: bool,
    copy: Option<String>,
    found: FoundFor,
    index: Option<(Arc<Tree>, Arc<Index>)>,
    pub session: Session,
    /// Whether the session has seen a tree since the last connection.
    session_live: bool,
    session_profile: Option<Arc<Profile>>,
    pub started: Instant,
    pub bar: bool,
    settings_read: bool,
    /// Whether the window was open at the last frame: the key that opens it also types its
    /// letter, which must not land in the search box.
    open_last_frame: bool,
    /// Laid-out panels: the planner's for a plan view, the statistics for what they show.
    pub plan_lines: Option<(Arc<PlanView>, Arc<Panel>)>,
    pub stats_lines: Option<((u64, u64, bool), Arc<Panel>)>,
    /// The buttons and panels drawn last frame, where clicks are looked up.
    pub hits: Vec<(Button, [f32; 4])>,
    pub areas: Vec<[f32; 4]>,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            query: String::new(),
            only_available: false,
            planning: false,
            stats: false,
            current: None,
            plan: Plan::default(),
            plan_revision: 0,
            plan_view: None,
            message: None,
            applying: VecDeque::new(),
            paste: false,
            copy: None,
            found: FoundFor::default(),
            index: None,
            session: Session::default(),
            session_live: false,
            session_profile: None,
            started: Instant::now(),
            bar: false,
            settings_read: false,
            open_last_frame: false,
            plan_lines: None,
            stats_lines: None,
            hits: Vec::new(),
            areas: Vec::new(),
        }
    }
}

static WINDOW: LazyLock<Mutex<Window>> = LazyLock::new(Mutex::default);

pub fn window() -> MutexGuard<'static, Window> {
    WINDOW.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Window {
    pub fn say(&mut self, text: impl Into<String>, colour: u32) {
        self.message = Some((text.into(), colour, Instant::now()));
    }

    /// The message to show now, if one is fresh.
    pub fn message(&self) -> Option<(&str, u32)> {
        self.message
            .as_ref()
            .filter(|(_, _, at)| at.elapsed() < MESSAGE_FOR)
            .map(|(text, colour, _)| (text.as_str(), *colour))
    }

    fn index_for(&mut self, tree: &Arc<Tree>, look: &Look) -> Arc<Index> {
        if let Some((known, index)) = &self.index
            && Arc::ptr_eq(known, tree)
        {
            return index.clone();
        }
        let index = Arc::new(Index::new(tree, look));
        self.index = Some((tree.clone(), index.clone()));
        index
    }

    /// The matches of the search for this tree and profile, worked out again only when the
    /// query, the filter, the tree or (with the filter on) the profile changed.
    pub fn found(
        &mut self,
        tree: &Arc<Tree>,
        look: &Look,
        profile: &Arc<Profile>,
        shown: &Shown,
    ) -> Arc<Found> {
        let same_tree = self
            .found
            .tree
            .as_ref()
            .is_some_and(|t| Arc::ptr_eq(t, tree));
        let same_profile = !self.only_available
            || self
                .found
                .profile
                .as_ref()
                .is_some_and(|p| Arc::ptr_eq(p, profile));
        if same_tree
            && same_profile
            && self.found.query == self.query
            && self.found.only_available == self.only_available
        {
            return self.found.found.clone();
        }
        let query = if plan::looks_like_share(&self.query) {
            String::new()
        } else {
            self.query.clone()
        };
        // The index is built on the first search, not when the window opens.
        let found = if find::fold(&query).is_empty() && !self.only_available {
            Arc::new(Found::default())
        } else {
            let index = self.index_for(tree, look);
            Arc::new(index.find(&query, self.only_available, shown))
        };
        self.found = FoundFor {
            tree: Some(tree.clone()),
            profile: Some(profile.clone()),
            query: self.query.clone(),
            only_available: self.only_available,
            found: found.clone(),
        };
        if self.current.is_some_and(|c| c >= found.ids.len()) {
            self.current = None;
        }
        found
    }

    /// The planner's view, fitted again to a new profile or tree.
    pub fn plan_view(&mut self, tree: &Arc<Tree>, profile: &Arc<Profile>) -> Arc<PlanView> {
        if let Some((t, p, revision, view)) = &self.plan_view
            && Arc::ptr_eq(t, tree)
            && *revision == self.plan_revision
        {
            if Arc::ptr_eq(p, profile) {
                return view.clone();
            }
            // Experience moved and no rank: the path stands, only the points changed.
            // The empty profile a connection starts with is not the player's: the plan waits.
            if p.ranks == profile.ranks || profile.ranks.is_empty() {
                let view = Arc::new(view.with_cost(self.plan.cost(tree, profile)));
                self.plan_view = Some((
                    tree.clone(),
                    profile.clone(),
                    self.plan_revision,
                    view.clone(),
                ));
                return view;
            }
        }
        let fitted = if profile.ranks.is_empty() {
            self.plan.clone()
        } else {
            self.plan.refit(tree, profile)
        };
        if fitted != self.plan {
            self.plan = fitted;
            self.plan_revision += 1;
        }
        let view = Arc::new(PlanView::new(tree, profile, &self.plan));
        self.plan_view = Some((
            tree.clone(),
            profile.clone(),
            self.plan_revision,
            view.clone(),
        ));
        view
    }

    fn set_plan(&mut self, next: Plan) {
        if next != self.plan {
            self.plan = next;
            self.plan_revision += 1;
        }
    }

    fn set_bar(&mut self, on: bool) {
        self.bar = on;
        self.stats_lines = None;
        save_settings(on);
    }
}

/// The tree, its look, the profile and what the window shows of it, from a snapshot of the
/// client state; `None` before the tree arrived.
fn snapshot() -> Option<(Arc<Tree>, Arc<Look>, Arc<Profile>, Arc<Shown>)> {
    let (tree, profile) = {
        let view = client();
        (view.tree.clone()?, view.profile.clone())
    };
    let look = art::look_for(&tree);
    let shown = ui::shown_for(&tree, &look, &profile);
    Some((tree, look, profile, shown))
}

fn is_ours(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(ID)
}

fn types_a_character(key: &str) -> bool {
    let Some(name) = key.strip_prefix("key.keyboard.") else {
        return false;
    };
    let keypad = name.strip_prefix("keypad.").is_some_and(|k| {
        k.len() == 1
            || [
                "divide", "multiply", "subtract", "add", "period", "equal", "comma",
            ]
            .contains(&k)
    });
    name.chars().count() == 1 || keypad || TYPING.contains(&name)
}

// Keys and typing while the window is open.

/// `native_screen_key_pressed_pre`: the search box, the jumps between matches, copy and paste.
pub fn on_key(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if *canceled || !is_ours(game, screen) {
        return;
    }
    let control = modifiers & CONTROL != 0 && modifiers & ALT == 0;
    let mut state = window();
    if control && key == PASTE {
        state.paste = true;
        *canceled = true;
        return;
    }
    if control && key == COPY {
        drop(state);
        copy_build();
        *canceled = true;
        return;
    }
    if key == ESCAPE {
        if state.stats {
            state.stats = false;
        } else if !state.query.is_empty() {
            state.query.clear();
            state.current = None;
        } else if state.only_available {
            state.only_available = false;
        } else {
            // Nothing to clear: the window closes as usual.
            return;
        }
        *canceled = true;
        return;
    }
    if key == BACKSPACE {
        if control {
            state.query.clear();
        } else {
            state.query.pop();
        }
        state.current = None;
        *canceled = true;
        return;
    }
    let step = if ENTER.contains(&key) {
        if plan::looks_like_share(&state.query) {
            let text = std::mem::take(&mut state.query);
            drop(state);
            import(&text);
            *canceled = true;
            return;
        }
        Some(if modifiers & SHIFT != 0 { -1 } else { 1 })
    } else if key == DOWN {
        Some(1)
    } else if key == UP {
        Some(-1)
    } else {
        None
    };
    if let Some(step) = step {
        drop(state);
        jump(step);
        *canceled = true;
        return;
    }
    if !control && types_a_character(key) {
        *canceled = true;
    }
}

/// `native_screen_char_typed_pre`: a character for the search box.
pub fn on_char(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if *canceled || !is_ours(game, screen) || c.is_control() {
        return;
    }
    *canceled = true;
    let mut state = window();
    if !state.open_last_frame {
        return;
    }
    let limit = if plan::looks_like_share(&state.query) {
        plan::MAX_SHARE_LEN
    } else {
        find::MAX_QUERY
    };
    if state.query.chars().count() < limit && !(state.query.is_empty() && c == ' ') {
        state.query.push(c);
        state.current = None;
    }
}

/// Moves the camera to the next match (`step` 1) or the one before (-1).
fn jump(step: isize) {
    let Some((tree, look, profile, shown)) = snapshot() else {
        return;
    };
    let target = {
        let mut state = window();
        let found = state.found(&tree, &look, &profile, &shown);
        let Some(index) = found.step(state.current, step) else {
            if found.active {
                state.say("Ничего не найдено", RED);
            }
            return;
        };
        state.current = Some(index);
        found.ids[index]
    };
    let Some(node) = tree.nodes.get(&target) else {
        return;
    };
    // Zoomed in far enough that frames are drawn, so the match is easy to see.
    let wanted = (30.0 / look.spacing.max(1.0)).clamp(view::MIN_ZOOM, view::MAX_ZOOM);
    let mut view = client();
    view.camera.zoom = view.camera.zoom.max(wanted);
    view.camera.pan = [-node.x, -node.y];
}

/// `native_key` outside the window: the experience bar's key.
pub fn on_game_key(game: &Game<'_>, key: &str, pressed: bool) {
    // An unbound key reads as "unknown", and so may a key the game has no name for.
    if pressed && key != "key.keyboard.unknown" && keys::is(game, key, keys::Key::XpBar) {
        let mut state = window();
        let on = !state.bar;
        state.set_bar(on);
    }
}

// The mouse, asked by ui.rs before the tree.

/// A click on one of the tools; true when the tools took it.
pub fn click(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    match crate::game::pct_window_draw::button_at(game, screen) {
        Some(button) => {
            press(button);
            true
        }
        None => blocks_tree(game, screen),
    }
}

/// Whether the tools keep the mouse from the tree: the statistics page is open, or the mouse is
/// over a panel or the tool row.
pub fn blocks_tree(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    let state = window();
    state.stats || crate::game::pct_window_draw::covers(game, screen, &state)
}

fn press(button: Button) {
    match button {
        Button::Available => {
            let mut state = window();
            state.only_available = !state.only_available;
            state.current = None;
        }
        Button::Planner => {
            let mut state = window();
            state.planning = !state.planning;
            if state.planning {
                state.say(
                    "Планировщик: ЛКМ добавляет ранг в план, ПКМ убирает узел",
                    AQUA,
                );
            }
        }
        Button::Stats => {
            let mut state = window();
            state.stats = !state.stats;
        }
        Button::Close => window().stats = false,
        Button::Bar => {
            let mut state = window();
            let on = !state.bar;
            state.set_bar(on);
        }
        Button::Clear => {
            let mut state = window();
            state.set_plan(Plan::default());
            state.applying.clear();
            state.say("План очищен", GREY);
        }
        Button::Apply => apply(),
        Button::Copy => copy_build(),
    }
}

/// A left click on node `id`: in the planner it goes into the plan instead of being learned.
pub fn plan_click(id: u16) -> bool {
    if !window().planning {
        return false;
    }
    let Some((tree, _, profile, _)) = snapshot() else {
        return true;
    };
    let mut state = window();
    let view = state.plan_view(&tree, &profile);
    match state.plan.with(&tree, &profile, id) {
        Some(next) => state.set_plan(next),
        None => {
            let full = tree
                .nodes
                .get(&id)
                .is_some_and(|node| view.planned(id) + profile.rank(id) >= node.max_rank());
            let why = if full {
                "Этот узел уже полностью в плане"
            } else {
                "Сначала добавьте в план соседний узел до конца"
            };
            state.say(why, RED);
        }
    }
    true
}

/// A right click on node `id`: in the planner it leaves the plan instead of switching.
pub fn plan_remove(id: u16) -> bool {
    if !window().planning {
        return false;
    }
    if let Some((tree, _, profile, _)) = snapshot() {
        let mut state = window();
        let next = state.plan.without(&tree, &profile, id);
        state.set_plan(next);
    }
    true
}

/// Sends the ranks of the plan that the points pay for now; the server checks each one.
fn apply() {
    let Some((tree, _, profile, _)) = snapshot() else {
        return;
    };
    let mut state = window();
    let view = state.plan_view(&tree, &profile);
    if view.steps == 0 {
        state.say("План пуст", GREY);
        return;
    }
    let affordable = view.cost.affordable;
    if affordable == 0 {
        state.say("Очков не хватает ни на один ранг плана", RED);
        return;
    }
    let steps: Vec<u16> = state.plan.steps()[..affordable].to_vec();
    state.applying = steps.into();
    let text = if affordable < view.steps {
        format!(
            "Изучаю {affordable} из {} рангов плана, на остальные не хватает очков",
            view.steps
        )
    } else {
        format!("Изучаю план: {affordable} {}", ranks_word(affordable))
    };
    state.say(text, GREEN);
}

/// Puts the build (the ranks learned and the plan after them) on the clipboard.
fn copy_build() {
    let Some((tree, _, profile, _)) = snapshot() else {
        return;
    };
    let mut state = window();
    let view = state.plan_view(&tree, &profile);
    let mut steps = plan::learned_steps(&tree, &profile);
    steps.extend_from_slice(state.plan.steps());
    let text = plan::encode(rules::PROFILE_LAYOUT, &steps);
    let shown: String = text.chars().take(24).collect();
    state.say(
        format!(
            "Строка сборки скопирована: {shown}… ({} {}, из них в плане {})",
            steps.len(),
            ranks_word(steps.len()),
            view.steps
        ),
        GREEN,
    );
    state.copy = Some(text);
}

/// Reads a share string into the plan, fitted to the player's profile.
fn import(text: &str) {
    let Some((tree, _, profile, _)) = snapshot() else {
        return;
    };
    let mut state = window();
    if profile.ranks.is_empty() {
        state.say(
            "Профиль ещё не пришёл с сервера, попробуйте через секунду",
            RED,
        );
        return;
    }
    match plan::decode(text, &tree, rules::PROFILE_LAYOUT) {
        Ok(steps) => {
            let (next, skipped) = Plan::fitted(&tree, &profile, &steps);
            let planned = next.steps().len();
            state.set_plan(next);
            state.planning = true;
            let mut words = format!(
                "Сборка загружена: в плане {planned} {}",
                ranks_word(planned)
            );
            if skipped.learned > 0 {
                words.push_str(&format!(", уже изучено {}", skipped.learned));
            }
            if skipped.unreachable > 0 {
                words.push_str(&format!(", не подошло {}", skipped.unreachable));
            }
            state.say(words, GREEN);
        }
        Err(error) => state.say(error.message(), RED),
    }
}

/// "ранг", "ранга" or "рангов" for `count`.
pub fn ranks_word(count: usize) -> &'static str {
    match (count % 10, count % 100) {
        (1, n) if n != 11 => "ранг",
        (2..=4, n) if !(12..=14).contains(&n) => "ранга",
        _ => "рангов",
    }
}

/// The line a tooltip gets in the planner.
pub fn tooltip_line(tree: &Arc<Tree>, profile: &Arc<Profile>, id: u16) -> Option<(String, u32)> {
    let mut state = window();
    if !state.planning {
        return None;
    }
    let view = state.plan_view(tree, profile);
    let planned = view.planned(id);
    let node = tree.nodes.get(&id)?;
    let line = if planned > 0 {
        format!("В плане: +{planned}. ПКМ: убрать из плана")
    } else if profile.rank(id) >= node.max_rank() {
        "Изучено".to_owned()
    } else {
        "Клик: добавить в план".to_owned()
    };
    Some((line, AQUA))
}

// Work that needs the game: the clipboard, the plan's requests, the session.

/// Every frame on the client.
pub fn on_frame(game: &mut Game<'_>) {
    read_settings_once();
    let open = game.native_mod_screen() == Some(ID);
    let (paste, copy, sends) = {
        let mut state = window();
        state.open_last_frame = open;
        let take = state.applying.len().min(APPLY_PER_FRAME);
        let sends: Vec<u16> = state.applying.drain(..take).collect();
        (std::mem::take(&mut state.paste), state.copy.take(), sends)
    };
    if let Some(text) = copy {
        game.clipboard.set(&text);
    }
    if paste {
        let text = game.clipboard.get();
        pasted(&text);
    }
    if !sends.is_empty() {
        client().requests.extend(sends);
    }
    observe_session();
}

/// Text from the clipboard: a share string goes to the planner, anything else to the search.
fn pasted(text: &str) {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if plan::looks_like_share(line) {
        import(line);
        return;
    }
    let mut state = window();
    for c in line.chars().filter(|c| !c.is_control()) {
        if state.query.chars().count() >= find::MAX_QUERY {
            break;
        }
        state.query.push(c);
    }
    state.current = None;
}

/// Feeds each new profile to the session; a new connection starts a new session.
fn observe_session() {
    let (tree_known, profile) = {
        let view = client();
        (view.tree.is_some(), view.profile.clone())
    };
    let mut state = window();
    if !tree_known {
        // A new connection: ranks queued for the old one are not sent.
        state.applying.clear();
        if state.session_live {
            state.session = Session::default();
            state.session_live = false;
            state.session_profile = None;
            state.started = Instant::now();
            state.stats_lines = None;
        }
        return;
    }
    // The empty profile a connection starts with has no start rank: it is not the player's.
    if profile.ranks.is_empty()
        || state
            .session_profile
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, &profile))
    {
        return;
    }
    if !state.session_live {
        state.session_live = true;
        state.started = Instant::now();
    }
    state.session.observe(&profile.xp, profile.general_xp);
    state.session_profile = Some(profile);
}

/// `net_from_server`: the gains message for the statistics page.
pub fn on_server_message(channel: &str, data: &[u8]) {
    if channel != gains::CHANNEL {
        return;
    }
    if let Some(entries) = session::decode_gains(data) {
        window().session.add_gains(entries);
    }
}

/// Whether the bar is on and the branch it should show: the one that earned last.
pub fn bar_wanted() -> Option<Option<String>> {
    let state = window();
    state.bar.then(|| state.session.last_branch.clone())
}

fn settings_path() -> Option<std::path::PathBuf> {
    Some(Game::native_config_dir().ok()?.join(SETTINGS_FILE))
}

fn read_settings_once() {
    if window().settings_read {
        return;
    }
    let on = settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| {
            text.lines()
                .any(|line| line.trim() == format!("{BAR_SETTING}1"))
        });
    let mut state = window();
    state.settings_read = true;
    state.bar = on;
}

fn save_settings(bar: bool) {
    let Some(path) = settings_path() else {
        return;
    };
    let text = format!("{BAR_SETTING}{}\n", u8::from(bar));
    if let Err(error) = std::fs::write(&path, text) {
        log::warn!(
            "path_of_craft_tree: {} not written: {error}",
            path.display()
        );
    }
}
