// The tree's keys. They are listed in Options > Mods with their defaults, and a player can rebind
// them there; `native_mod_key` reads the player's choice, so other mods' keys never have to give way.
use crate::game::{Game, pct_glue as glue};
use crate::mod_api::NativeMod;

/// One of the tree's keys.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Tree,
    OreSense,
    Cartographer,
    TreasureSense,
    Home,
    PocketBench,
    Sharpener,
}

/// Each key: its name in the game's key list, its label in Options and its default.
const KEYS: [(Key, &str, &str, &str); 7] = [
    (Key::Tree, "tree", "Открыть дерево", "key.keyboard.k"),
    (Key::OreSense, "ore_sense", "Рудный нюх", "key.keyboard.v"),
    (
        Key::Cartographer,
        "cartographer",
        "Картограф",
        "key.keyboard.j",
    ),
    (
        Key::TreasureSense,
        "treasure_sense",
        "Чутьё сокровищ",
        "key.keyboard.i",
    ),
    (Key::Home, "home", "Возвращение домой", "key.keyboard.h"),
    (
        Key::PocketBench,
        "pocket_bench",
        "Карманный верстак",
        "key.keyboard.y",
    ),
    (Key::Sharpener, "sharpener", "Точильщик", "key.keyboard.u"),
];

const TITLE: &str = "Path-of-craft-tree";
const DESCRIPTION: &str = "Опыт профессий за то, чем вы занимаетесь, и дерево пассивных умений на \
                           995 узлов. Клавиша дерева и клавиши умений переназначаются здесь.";

/// Whether `pressed` (a key name such as `key.keyboard.k`) is the key the player chose for `which`.
pub fn is(game: &Game<'_>, pressed: &str, which: Key) -> bool {
    KEYS.iter()
        .find(|(key, ..)| *key == which)
        .is_some_and(|(_, name, _, default)| {
            pressed == game.native_mod_key(glue::ID, name, default)
        })
}

/// The tree's row in Options > Mods, with its keys.
pub fn on_mods(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: glue::ID.to_owned(),
        title: TITLE.to_owned(),
        description: DESCRIPTION.to_owned(),
        keys: KEYS
            .iter()
            .map(|(_, name, label, default)| {
                [
                    (*name).to_owned(),
                    (*label).to_owned(),
                    (*default).to_owned(),
                ]
            })
            .collect(),
    });
}
