// The server owner's settings file, config/path_of_craft_tree-server.json (settings_rules.rs): read
// when the server first needs it and again on `/pct admin reload`. A missing file is written with
// the defaults; a broken one is reported and the defaults are used. The experience multipliers go
// into the server's tree (glue.rs), group play reads its numbers from here.
use crate::game::pct_settings_rules::{self as rules, Group, Settings};
use crate::game::{Game, pct_glue as glue, pct_rules::Tree, pct_rules::XpScale};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

#[derive(Default)]
struct State {
    /// What the file says; None until it is first read.
    file: Option<Settings>,
    /// What the self-test runs with instead, while it runs.
    test: Option<Settings>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn path() -> Result<PathBuf, String> {
    Ok(Game::native_config_dir()?.join(rules::FILE_NAME))
}

/// Reads the file, writing the defaults first when there is none. Every problem is a warning.
fn read_file() -> (Settings, Vec<String>) {
    let path = match path() {
        Ok(path) => path,
        Err(error) => {
            return (
                Settings::default(),
                vec![format!("no config folder: {error}")],
            );
        }
    };
    if !path.exists() {
        return match glue::write_in_folder(&path, rules::template().as_bytes()) {
            Ok(()) => {
                log::info!(
                    "path_of_craft_tree: server settings written with the defaults: {}",
                    path.display()
                );
                (Settings::default(), Vec::new())
            }
            Err(error) => (
                Settings::default(),
                vec![format!("{} not written: {error}", path.display())],
            ),
        };
    }
    match std::fs::read(&path) {
        Ok(bytes) => rules::parse(&bytes),
        Err(error) => (
            Settings::default(),
            vec![format!(
                "{} not read ({error}): all settings are defaults",
                path.display()
            )],
        ),
    }
}

fn load() -> (Settings, Vec<String>) {
    let (settings, warnings) = read_file();
    for warning in &warnings {
        log::warn!("path_of_craft_tree: server settings: {warning}");
    }
    log::info!(
        "path_of_craft_tree: server settings: {}",
        rules::describe(&settings)
    );
    (settings, warnings)
}

/// The settings in use: the self-test's while it runs, else the file's, read on first use.
pub fn current() -> Settings {
    {
        let state = state();
        if let Some(settings) = state.test.as_ref().or(state.file.as_ref()) {
            return settings.clone();
        }
    }
    let (settings, _) = load();
    state().file.get_or_insert(settings).clone()
}

/// Group play's numbers in use.
pub fn group() -> Group {
    {
        let state = state();
        if let Some(settings) = state.test.as_ref().or(state.file.as_ref()) {
            return settings.group.clone();
        }
    }
    current().group
}

/// The multipliers for `tree`: the settings' ones for the branches it has. Unknown branches are
/// reported each time a tree takes them.
pub fn scale_for(tree: &Tree) -> XpScale {
    let (kept, warnings) = rules::known_branches(&current(), |b| tree.branches.contains_key(b));
    for warning in warnings {
        log::warn!("path_of_craft_tree: server settings: {warning}");
    }
    kept.scale
}

/// Reads the file again and puts its multipliers into the tree; what was wrong comes back with
/// the summary of what is now in use, for the command's answer.
pub fn reload() -> Vec<String> {
    let (settings, mut lines) = load();
    if let Some(loaded) = glue::current_tree() {
        let tree = &loaded.tree;
        let (_, unknown) = rules::known_branches(&settings, |b| tree.branches.contains_key(b));
        lines.extend(unknown);
    }
    let summary = rules::describe(&settings);
    state().file = Some(settings);
    glue::apply_settings();
    lines.push(summary);
    lines
}

/// Self-test only: runs with `settings` until it is dropped, then with what ran before.
pub struct TestSettings {
    before: Option<Settings>,
}

impl TestSettings {
    pub fn new(settings: Settings) -> Self {
        let before = state().test.replace(settings);
        glue::apply_settings();
        TestSettings { before }
    }
}

impl Drop for TestSettings {
    fn drop(&mut self) {
        state().test = self.before.take();
        glue::apply_settings();
    }
}
