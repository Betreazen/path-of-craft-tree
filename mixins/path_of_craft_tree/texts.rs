// The mod's English texts inside the game: the dictionary from data/en-*.json and the client's
// language. lang.rs holds the game-free part.

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::LazyLock;

use crate::game::pct_lang::{Dict, Lang, LangCell, english};

static EN: LazyLock<Dict> = LazyLock::new(|| {
    english(&[
        horizon_client::native_data!("path_of_craft_tree", "en-names.json"),
        horizon_client::native_data!("path_of_craft_tree", "en-texts.json"),
        horizon_client::native_data!("path_of_craft_tree", "en-ui.json"),
    ])
    .unwrap_or_else(|error| {
        log::error!("path_of_craft_tree: English texts not read, all stay Russian: {error}");
        Dict::default()
    })
});

static CLIENT: LangCell = LangCell::new();

/// The client's game language, read every frame from its settings.
pub fn on_client_frame(game: &crate::game::Game<'_>) {
    CLIENT.set(Lang::from_code(&game.settings.language));
}

pub fn client_lang() -> Lang {
    CLIENT.get()
}

/// A client text in this game's language.
pub fn tr(ru: &str) -> Cow<'_, str> {
    EN.text(CLIENT.get(), ru)
}

/// A client template in this game's language, its `{}` places filled with `args`.
pub fn trf(template: &str, args: &[&dyn Display]) -> String {
    EN.format(CLIENT.get(), template, args)
}

/// A player's language as their client told the server. The host of a single-player or LAN world
/// is no guest of its own server, so the game's own language stands in; a dedicated server never
/// draws a frame, so that stays Russian there.
pub fn player_lang(game: &crate::game::Game<'_>, player: u128) -> Lang {
    game.native_player_client(player)
        .map_or_else(|| CLIENT.get(), |info| Lang::from_code(&info.language))
}

/// A text for a player in `lang`.
pub fn tr_in(lang: Lang, ru: &str) -> Cow<'_, str> {
    EN.text(lang, ru)
}

/// A template for a player in `lang`.
pub fn trf_in(lang: Lang, template: &str, args: &[&dyn Display]) -> String {
    EN.format(lang, template, args)
}

/// The tree as the client shows it: names and texts in this game's language.
pub fn localized(tree: crate::game::pct_rules::Tree) -> crate::game::pct_rules::Tree {
    match CLIENT.get() {
        Lang::Ru => tree,
        lang => tree.translated(|ru| EN.text(lang, ru).into_owned()),
    }
}
