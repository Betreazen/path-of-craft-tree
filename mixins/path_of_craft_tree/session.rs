// The tree window's statistics, free of game types: experience earned this session by branch,
// worked out from the profiles the server sends, and the best sources from the server's gains
// message (`path_of_craft_tree:gains`). The message is new in 0.4.12 and needs no protocol
// change: a client that does not know it ignores it, and without it the page shows no sources.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The gains message's shape; a message of another version is ignored.
pub const GAINS_VERSION: u32 = 1;
/// Bounds for what a gains message may carry, so a broken server cannot flood the page.
const MAX_ENTRIES: usize = 256;
const MAX_NAME: usize = 96;
const MAX_AMOUNT: f64 = 1.0e9;
/// Distinct sources kept; past that the smallest give way.
const MAX_SOURCES: usize = 512;

/// The gains message: `[branch, source, experience]` earned since the last one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gains {
    pub v: u32,
    pub g: Vec<(String, String, f64)>,
}

/// The gains message for `entries`, at most `MAX_ENTRIES` of them, biggest first.
pub fn encode_gains(entries: &BTreeMap<(String, String), f64>) -> Vec<u8> {
    let mut g: Vec<(String, String, f64)> = entries
        .iter()
        .filter(|(_, xp)| xp.is_finite() && **xp > 0.0)
        .map(|((branch, source), xp)| (branch.clone(), source.clone(), *xp))
        .collect();
    g.sort_by(|a, b| b.2.total_cmp(&a.2));
    g.truncate(MAX_ENTRIES);
    serde_json::to_vec(&Gains {
        v: GAINS_VERSION,
        g,
    })
    .unwrap_or_default()
}

/// A gains message, with every entry checked; `None` for anything malformed.
pub fn decode_gains(bytes: &[u8]) -> Option<Vec<(String, String, f64)>> {
    let gains: Gains = serde_json::from_slice(bytes).ok()?;
    if gains.v != GAINS_VERSION || gains.g.len() > MAX_ENTRIES {
        return None;
    }
    let sane = |(branch, source, xp): &(String, String, f64)| {
        !branch.is_empty()
            && branch.len() <= MAX_NAME
            && !source.is_empty()
            && source.len() <= MAX_NAME
            && xp.is_finite()
            && *xp > 0.0
            && *xp <= MAX_AMOUNT
    };
    gains.g.iter().all(sane).then_some(gains.g)
}

/// Experience earned since the session began.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    /// The experience table seen last; `None` before the first profile.
    previous: Option<(BTreeMap<String, f64>, f64)>,
    /// By branch.
    pub gained: BTreeMap<String, f64>,
    pub general: f64,
    /// The branch that earned last, for the bar on the screen.
    pub last_branch: Option<String>,
    /// By branch and source, from the gains messages.
    sources: BTreeMap<(String, String), f64>,
    /// Grows with every change, so the window knows when to lay the page out again.
    pub revision: u64,
}

impl Session {
    /// A new profile arrived: what grew since the last one is earned this session. The first
    /// profile only sets where the session starts; a table that shrank (a server command) earns
    /// nothing.
    pub fn observe(&mut self, xp: &BTreeMap<String, f64>, general: f64) {
        let Some((before, before_general)) = &self.previous else {
            self.previous = Some((xp.clone(), general));
            return;
        };
        let mut best: Option<(&str, f64)> = None;
        let mut changed = false;
        for (branch, now) in xp {
            let grown = now - before.get(branch).copied().unwrap_or(0.0);
            if grown.is_finite() && grown > 0.0 {
                *self.gained.entry(branch.clone()).or_insert(0.0) += grown;
                changed = true;
                if best.is_none_or(|(_, most)| grown > most) {
                    best = Some((branch, grown));
                }
            }
        }
        let grown = general - before_general;
        if grown.is_finite() && grown > 0.0 {
            self.general += grown;
            changed = true;
        }
        if let Some((branch, _)) = best {
            self.last_branch = Some(branch.to_owned());
        }
        if changed || before != xp {
            self.previous = Some((xp.clone(), general));
            self.revision += 1;
        }
    }

    /// Adds a gains message's entries.
    pub fn add_gains(&mut self, entries: Vec<(String, String, f64)>) {
        for (branch, source, xp) in entries {
            *self.sources.entry((branch, source)).or_insert(0.0) += xp;
        }
        if self.sources.len() > MAX_SOURCES {
            let mut all: Vec<_> = std::mem::take(&mut self.sources).into_iter().collect();
            all.sort_by(|a, b| b.1.total_cmp(&a.1));
            all.truncate(MAX_SOURCES);
            self.sources = all.into_iter().collect();
        }
        self.revision += 1;
    }

    pub fn total(&self) -> f64 {
        self.gained.values().sum()
    }

    /// Branches by experience earned, most first.
    pub fn branches(&self) -> Vec<(&str, f64)> {
        let mut out: Vec<(&str, f64)> = self
            .gained
            .iter()
            .map(|(branch, xp)| (branch.as_str(), *xp))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        out
    }

    /// The `count` best sources over all branches: branch, source and experience.
    pub fn best_sources(&self, count: usize) -> Vec<(&str, &str, f64)> {
        let mut out: Vec<(&str, &str, f64)> = self
            .sources
            .iter()
            .map(|((branch, source), xp)| (branch.as_str(), source.as_str(), *xp))
            .collect();
        out.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.1.cmp(b.1)));
        out.truncate(count);
        out
    }

    /// A branch's best source and what it gave.
    pub fn best_source_of(&self, branch: &str) -> Option<(&str, f64)> {
        self.sources
            .iter()
            .filter(|((b, _), _)| b == branch)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|((_, source), xp)| (source.as_str(), *xp))
    }

    pub fn knows_sources(&self) -> bool {
        !self.sources.is_empty()
    }
}

/// Experience an hour at this pace; nothing for the first minute, which says little.
pub fn per_hour(xp: f64, seconds: f64) -> Option<f64> {
    (seconds >= 60.0).then(|| xp * 3600.0 / seconds)
}

/// The names of the actions experience is paid for (`act:*` in the tree).
const ACTIONS: [(&str, &str); 45] = [
    ("act:advancement", "достижения"),
    ("act:biome", "новые биомы"),
    ("act:block", "блоки"),
    ("act:bone_meal", "костная мука"),
    ("act:book", "книги"),
    ("act:boss", "боссы"),
    ("act:breed", "разведение"),
    ("act:brew", "зелья"),
    ("act:brew_first", "новые зелья"),
    ("act:brush", "кисточка"),
    ("act:build", "постройки"),
    ("act:cake", "торты"),
    ("act:campfire", "костёр"),
    ("act:chunk", "новые места"),
    ("act:cook", "готовка"),
    ("act:craft", "крафт"),
    ("act:craft_first", "новые рецепты"),
    ("act:craft_food", "еда на верстаке"),
    ("act:deco", "украшения"),
    ("act:dimension", "измерения"),
    ("act:eat", "еда"),
    ("act:enchant", "зачарование"),
    ("act:enchant_first", "новые чары"),
    ("act:fish", "рыба"),
    ("act:gear", "снаряжение"),
    ("act:hit", "удары"),
    ("act:honey", "мёд"),
    ("act:ingot", "слитки"),
    ("act:junk", "хлам с удочки"),
    ("act:kill", "мобы"),
    ("act:milk", "дойка"),
    ("act:mix", "смеситель"),
    ("act:nether_wart", "незерский нарост"),
    ("act:none", "прочее"),
    ("act:path", "тропинки"),
    ("act:repair", "починка"),
    ("act:shear", "стрижка"),
    ("act:shot", "стрельба"),
    ("act:stew", "супы"),
    ("act:strip", "окорка"),
    ("act:structure", "строения"),
    ("act:tame", "приручение"),
    ("act:trade", "торговля"),
    ("act:treasure", "сокровища с удочки"),
    ("act:upgrade", "улучшение"),
];

/// A source as the page shows it: an action's name, or a block's id from its loot table.
pub fn source_label(source: &str) -> String {
    source_label_with(source, |_| None)
}

/// As `source_label`, but a block is named by `block_name` (the game's own name for an id such as
/// `minecraft:gold_ore`) when it knows the block.
pub fn source_label_with(source: &str, block_name: impl Fn(&str) -> Option<String>) -> String {
    if let Some((_, name)) = ACTIONS.iter().find(|(id, _)| *id == source) {
        return (*name).to_owned();
    }
    let tail = source.rsplit(['/', ':']).next().unwrap_or(source);
    let Some((namespace, _)) = source.split_once(":blocks/") else {
        return tail.replace('_', " ");
    };
    let name =
        block_name(&format!("{namespace}:{tail}")).unwrap_or_else(|| tail.replace('_', " "));
    format!("блок: {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(entries: &[(&str, f64)]) -> BTreeMap<String, f64> {
        entries.iter().map(|(b, x)| ((*b).to_owned(), *x)).collect()
    }

    #[test]
    fn the_first_profile_starts_the_session_and_later_growth_counts() {
        let mut session = Session::default();
        session.observe(&table(&[("mining", 500.0)]), 100.0);
        assert_eq!(session.total(), 0.0);
        assert_eq!(session.last_branch, None);
        session.observe(&table(&[("mining", 520.0), ("fishing", 3.0)]), 110.0);
        session.observe(&table(&[("mining", 530.0), ("fishing", 3.0)]), 115.0);
        assert_eq!(session.gained["mining"], 30.0);
        assert_eq!(session.gained["fishing"], 3.0);
        assert_eq!(session.general, 15.0);
        assert_eq!(session.last_branch.as_deref(), Some("mining"));
        assert_eq!(session.branches(), vec![("mining", 30.0), ("fishing", 3.0)]);
        let revision = session.revision;
        session.observe(&table(&[("mining", 530.0), ("fishing", 3.0)]), 115.0);
        assert_eq!(session.revision, revision, "the same table changes nothing");
        // A table that shrank earns nothing and becomes the new reference.
        session.observe(&table(&[("mining", 10.0), ("fishing", 3.0)]), 115.0);
        session.observe(&table(&[("mining", 15.0), ("fishing", 3.0)]), 115.0);
        assert_eq!(session.gained["mining"], 35.0);
    }

    #[test]
    fn gains_round_trip_and_rank_the_sources() {
        let mut entries = BTreeMap::new();
        entries.insert(
            ("mining".to_owned(), "minecraft:blocks/stone".to_owned()),
            40.0,
        );
        entries.insert(
            ("mining".to_owned(), "minecraft:blocks/coal_ore".to_owned()),
            90.0,
        );
        entries.insert(("fishing".to_owned(), "act:fish".to_owned()), 60.0);
        entries.insert(("fishing".to_owned(), "act:junk".to_owned()), 0.0);
        let bytes = encode_gains(&entries);
        let decoded = decode_gains(&bytes).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].2, 90.0);
        let mut session = Session::default();
        session.add_gains(decoded.clone());
        session.add_gains(decoded);
        assert!(session.knows_sources());
        assert_eq!(
            session.best_sources(2),
            vec![
                ("mining", "minecraft:blocks/coal_ore", 180.0),
                ("fishing", "act:fish", 120.0)
            ]
        );
        assert_eq!(session.best_source_of("fishing"), Some(("act:fish", 120.0)));
        assert_eq!(session.best_source_of("archery"), None);
    }

    #[test]
    fn malformed_gains_are_ignored() {
        let bad = [
            &br#"{"v":2,"g":[]}"#[..],
            br#"{"v":1,"g":[["","act:fish",1.0]]}"#,
            br#"{"v":1,"g":[["mining","act:fish",-1.0]]}"#,
            br#"{"v":1,"g":[["mining","act:fish",1e300]]}"#,
            br#"{"v":1,"g":[["mining","act:fish"]]}"#,
            b"not json",
        ];
        for bytes in bad {
            assert_eq!(
                decode_gains(bytes),
                None,
                "{}",
                String::from_utf8_lossy(bytes)
            );
        }
        let long = format!(r#"{{"v":1,"g":[["mining","{}",1.0]]}}"#, "x".repeat(200));
        assert_eq!(decode_gains(long.as_bytes()), None);
        assert_eq!(decode_gains(br#"{"v":1,"g":[]}"#), Some(vec![]));
    }

    #[test]
    fn sources_get_readable_names() {
        assert_eq!(source_label("act:fish"), "рыба");
        assert_eq!(
            source_label("minecraft:blocks/diamond_ore"),
            "блок: diamond ore"
        );
        assert_eq!(source_label("something"), "something");
        let game_names = |id: &str| (id == "minecraft:gold_ore").then(|| "Золотая руда".to_owned());
        assert_eq!(
            source_label_with("minecraft:blocks/gold_ore", game_names),
            "блок: Золотая руда"
        );
        assert_eq!(
            source_label_with("minecraft:blocks/diamond_ore", game_names),
            "блок: diamond ore"
        );
        assert_eq!(source_label_with("act:fish", game_names), "рыба");
        assert_eq!(per_hour(100.0, 30.0), None);
        assert_eq!(per_hour(100.0, 1800.0), Some(200.0));
    }
}
