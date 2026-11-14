// Search in the tree window, free of game types: node names and effect texts matched against
// what the player typed, the matches ordered from the start outward, and the filter of nodes a
// player can take right now. The window keeps one `Found` per query and profile, so a frame only
// reads it.
use crate::game::pct_look::{Look, Shown, State};
use crate::game::pct_rules::{Kind, Tree};

/// The longest query the search box takes, in characters.
pub const MAX_QUERY: usize = 48;

/// Text as the search compares it: lower case, `ё` read as `е`, runs of spaces as one.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = if c == 'ё' { 'е' } else { c };
        if c.is_whitespace() {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Every node's searchable text, folded once per tree, nearest the start first.
pub struct Index {
    entries: Vec<(u16, String)>,
    /// One past the highest node id, for the match mask.
    size: usize,
}

impl Index {
    pub fn new(tree: &Tree, look: &Look) -> Index {
        let mut entries: Vec<(u16, String)> = tree
            .nodes
            .values()
            .filter(|node| node.kind != Kind::Start)
            .map(|node| {
                let mut text = node.name.clone();
                for part in std::iter::once(&node.description).chain(&node.rank_descriptions) {
                    text.push('\n');
                    text.push_str(part);
                }
                (node.id, fold(&text))
            })
            .collect();
        let distance = |id: &u16| look.dist.get(id).copied().unwrap_or(f32::INFINITY);
        entries.sort_by(|a, b| {
            distance(&a.0)
                .total_cmp(&distance(&b.0))
                .then(a.0.cmp(&b.0))
        });
        let size = tree
            .nodes
            .keys()
            .last()
            .map_or(0, |id| usize::from(*id) + 1);
        Index { entries, size }
    }

    /// The nodes whose text holds every word of `query`, and that the player can take now when
    /// `only_available` is set; nearest the start first. An empty query with the filter off finds
    /// nothing: there is nothing to look for.
    pub fn find(&self, query: &str, only_available: bool, shown: &Shown) -> Found {
        let folded = fold(query);
        let words: Vec<&str> = folded.split(' ').filter(|w| !w.is_empty()).collect();
        let mut found = Found {
            ids: Vec::new(),
            mask: vec![false; self.size],
            active: !words.is_empty() || only_available,
        };
        if !found.active {
            return found;
        }
        for (id, text) in &self.entries {
            if only_available && shown.state(*id) != State::Available {
                continue;
            }
            if words.iter().all(|word| text.contains(word)) {
                found.ids.push(*id);
                found.mask[usize::from(*id)] = true;
            }
        }
        found
    }
}

/// What the search found: the matches in jump order, and a mask by node id for drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Found {
    pub ids: Vec<u16>,
    mask: Vec<bool>,
    /// Whether a search or the filter is on: the window then dims everything that did not match.
    pub active: bool,
}

impl Found {
    pub fn contains(&self, id: u16) -> bool {
        self.mask.get(usize::from(id)).copied().unwrap_or(false)
    }

    /// The match `step` places after `current` (before it when negative), going round.
    pub fn step(&self, current: Option<usize>, step: isize) -> Option<usize> {
        let count = self.ids.len() as isize;
        if count == 0 {
            return None;
        }
        let next = match current {
            Some(index) => index as isize + step,
            None if step < 0 => count - 1,
            None => 0,
        };
        Some(next.rem_euclid(count) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = r##"{
        "start": 0, "general_share": 0.5, "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 5},
        "branches": {"a": {"name": "A"}},
        "base_attributes": {},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "start", "name": "Старт", "kind": "start", "links": [1]},
            {"id": 1, "x": 0, "y": -40, "icon": "i", "name": "Рудный Нюх", "kind": "big", "branch": "a",
             "description": "Подсвечивает руду вокруг", "links": [2]},
            {"id": 2, "x": 0, "y": -80, "icon": "i", "name": "Ёмкий мешок", "kind": "small", "branch": "a",
             "description": "Больше места", "links": [3], "ranks": 2,
             "rank_descriptions": ["+1 ячейка", "+2 ячейки руды"]},
            {"id": 3, "x": 0, "y": -120, "icon": "i", "name": "Удар", "kind": "small", "branch": "a"}
        ]
    }"##;

    fn setup() -> (Tree, Look) {
        let tree = Tree::parse(TREE.as_bytes()).unwrap();
        let look = Look::new(&tree);
        (tree, look)
    }

    #[test]
    fn folding_lowers_case_reads_yo_as_ye_and_squeezes_spaces() {
        assert_eq!(fold("  Ёмкий   МЕШОК "), "емкий мешок");
        assert_eq!(fold(""), "");
    }

    #[test]
    fn every_word_must_match_in_names_or_effect_texts() {
        let (tree, look) = setup();
        let shown = Shown::new(&tree, &look, &tree.new_profile());
        let index = Index::new(&tree, &look);
        assert_eq!(index.find("руд", false, &shown).ids, vec![1, 2]);
        assert_eq!(index.find("емкий", false, &shown).ids, vec![2]);
        assert_eq!(index.find("ячейки руды", false, &shown).ids, vec![2]);
        assert_eq!(index.find("НЮХ руду", false, &shown).ids, vec![1]);
        assert!(index.find("старт", false, &shown).ids.is_empty());
        let nothing = index.find("   ", false, &shown);
        assert!(!nothing.active && nothing.ids.is_empty());
    }

    #[test]
    fn the_filter_keeps_what_can_be_taken_now() {
        let (tree, look) = setup();
        let mut profile = tree.new_profile();
        profile.xp.insert("a".to_owned(), 100.0);
        let shown = Shown::new(&tree, &look, &profile);
        let index = Index::new(&tree, &look);
        let found = index.find("", true, &shown);
        assert!(found.active);
        assert_eq!(found.ids, vec![1]);
        assert!(found.contains(1) && !found.contains(2) && !found.contains(999));
        assert!(index.find("мешок", true, &shown).ids.is_empty());
        let poor = Shown::new(&tree, &look, &tree.new_profile());
        assert!(index.find("", true, &poor).ids.is_empty());
    }

    #[test]
    fn stepping_goes_round_both_ways() {
        let found = Found {
            ids: vec![4, 5, 6],
            mask: Vec::new(),
            active: true,
        };
        assert_eq!(found.step(None, 1), Some(0));
        assert_eq!(found.step(None, -1), Some(2));
        assert_eq!(found.step(Some(2), 1), Some(0));
        assert_eq!(found.step(Some(0), -1), Some(2));
        assert_eq!(Found::default().step(Some(1), 1), None);
    }
}
