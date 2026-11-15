// The tree window's planner, free of game types: a path of ranks marked without spending points,
// what it costs in each branch's points and in general ones, and the short share string a build
// travels in. Applying a plan sends its ranks one by one as ordinary requests; the server checks
// each of them as it checks a click.
use crate::game::pct_rules::{Kind, Profile, Ranks, Tree};
use std::collections::BTreeMap;

/// The share string's shape. A string of another version is refused with a clear message.
pub const SHARE_VERSION: u8 = 1;
const SHARE_PREFIX: &str = "PCT";
/// The most ranks a plan holds: more than the whole tree has.
pub const MAX_STEPS: usize = 4096;
/// The longest share string read, so a pasted book does not stall a frame.
pub const MAX_SHARE_LEN: usize = 16_384;
/// Ranks of one node merged into one entry of the share string, at most.
const RUN: u8 = 4;

/// Ranks to learn, in order; a node with two ranks to go appears twice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    steps: Vec<u16>,
}

/// Why a plan step was left out when a plan was fitted to a profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Skipped {
    /// The rank is learned already.
    pub learned: usize,
    /// No full neighbour leads to it, or the tree has no such node.
    pub unreachable: usize,
}

/// One branch's part of a plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BranchCost {
    /// Ranks of the branch's nodes in the plan.
    pub planned: u32,
    /// The branch's own points the player has now.
    pub own: u32,
    /// Ranks neither the branch's points nor general ones pay for.
    pub missing: u32,
}

/// What a plan costs a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cost {
    pub ranks: u32,
    /// By branch id; nodes outside every branch are under "".
    pub branches: BTreeMap<String, BranchCost>,
    /// General points the plan takes, and those the player has.
    pub general_used: u32,
    pub general_have: u32,
    pub missing: u32,
    /// How many steps from the first the points now pay for: applying sends these.
    pub affordable: usize,
}

/// Ranks by node id in flat arrays, so replaying a long plan looks no map up per link.
struct Sim<'a> {
    tree: &'a Tree,
    rank: Vec<u8>,
    /// Each node's highest rank; 0 for ids the tree does not have.
    max: Vec<u8>,
}

impl<'a> Sim<'a> {
    fn new(tree: &'a Tree, ranks: &Ranks) -> Sim<'a> {
        let size = tree
            .nodes
            .keys()
            .last()
            .map_or(0, |id| usize::from(*id) + 1);
        let mut max = vec![0; size];
        for node in tree.nodes.values() {
            max[usize::from(node.id)] = node.max_rank();
        }
        let mut rank = vec![0; size];
        for (id, r) in ranks {
            if let Some(slot) = rank.get_mut(usize::from(*id)) {
                *slot = *r;
            }
        }
        Sim { tree, rank, max }
    }

    fn rank(&self, id: u16) -> u8 {
        self.rank.get(usize::from(id)).copied().unwrap_or(0)
    }

    fn full(&self, id: u16) -> bool {
        let i = usize::from(id);
        self.max
            .get(i)
            .is_some_and(|max| *max > 0 && self.rank[i] >= *max)
    }

    /// Whether one more rank of `id` fits: the node is there, is not full and has a full
    /// neighbour, as the rules' `Tree::is_reachable` asks. `Err(true)` when it is full already.
    fn fits(&self, id: u16) -> Result<(), bool> {
        let Some(node) = self.tree.nodes.get(&id) else {
            return Err(false);
        };
        if node.kind == Kind::Start || self.rank(id) >= node.max_rank() {
            return Err(true);
        }
        if node.links.iter().any(|link| self.full(*link)) {
            Ok(())
        } else {
            Err(false)
        }
    }

    /// The first full neighbour of `id`: the way the plan reaches it.
    fn parent(&self, id: u16) -> Option<u16> {
        let node = self.tree.nodes.get(&id)?;
        node.links.iter().copied().find(|link| self.full(*link))
    }

    fn learn(&mut self, id: u16) {
        if let Some(rank) = self.rank.get_mut(usize::from(id)) {
            *rank = rank.saturating_add(1);
        }
    }
}

impl Plan {
    pub fn steps(&self) -> &[u16] {
        &self.steps
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// `steps` fitted to `profile`: each one is kept when it fits the profile with the steps
    /// kept before it; the rest are counted by why they were left out.
    pub fn fitted(tree: &Tree, profile: &Profile, steps: &[u16]) -> (Plan, Skipped) {
        let mut sim = Sim::new(tree, &profile.ranks);
        let mut kept = Vec::with_capacity(steps.len().min(MAX_STEPS));
        let mut skipped = Skipped::default();
        for &id in steps.iter().take(MAX_STEPS) {
            match sim.fits(id) {
                Ok(()) => {
                    sim.learn(id);
                    kept.push(id);
                }
                Err(true) => skipped.learned += 1,
                Err(false) => skipped.unreachable += 1,
            }
        }
        (Plan { steps: kept }, skipped)
    }

    /// The plan fitted again to a new profile: ranks learned since then drop out.
    pub fn refit(&self, tree: &Tree, profile: &Profile) -> Plan {
        Plan::fitted(tree, profile, &self.steps).0
    }

    /// The plan with one more rank of `id`, when it fits after the plan; points do not matter.
    pub fn with(&self, tree: &Tree, profile: &Profile, id: u16) -> Option<Plan> {
        if self.steps.len() >= MAX_STEPS {
            return None;
        }
        let mut sim = Sim::new(tree, &profile.ranks);
        for &step in &self.steps {
            sim.learn(step);
        }
        sim.fits(id).ok()?;
        let mut steps = self.steps.clone();
        steps.push(id);
        Some(Plan { steps })
    }

    /// The plan without `id`, and without the steps that only `id` led to.
    pub fn without(&self, tree: &Tree, profile: &Profile, id: u16) -> Plan {
        let rest: Vec<u16> = self.steps.iter().copied().filter(|s| *s != id).collect();
        Plan::fitted(tree, profile, &rest).0
    }

    /// Planned ranks by node.
    pub fn planned(&self) -> BTreeMap<u16, u8> {
        let mut planned = BTreeMap::new();
        for &id in &self.steps {
            *planned.entry(id).or_insert(0u8) += 1;
        }
        planned
    }

    /// For each step, the neighbour it is reached through: the first full one when it is taken.
    pub fn parents(&self, tree: &Tree, profile: &Profile) -> Vec<(u16, u16)> {
        let mut sim = Sim::new(tree, &profile.ranks);
        let mut out = Vec::new();
        for &id in &self.steps {
            if sim.rank(id) == 0
                && let Some(parent) = sim.parent(id)
            {
                out.push((parent, id));
            }
            sim.learn(id);
        }
        out
    }

    /// What the plan costs `profile`, paid in order as the server would: a branch's own points
    /// first, general points after them.
    pub fn cost(&self, tree: &Tree, profile: &Profile) -> Cost {
        let mut left = tree.points(profile);
        let general_have = left.general;
        let mut cost = Cost {
            general_have,
            affordable: self.steps.len(),
            ..Cost::default()
        };
        // Keyed by the tree's own names while counting; copied once at the end.
        let mut branches: BTreeMap<&str, BranchCost> = BTreeMap::new();
        for (index, id) in self.steps.iter().enumerate() {
            let Some(node) = tree.nodes.get(id) else {
                continue;
            };
            cost.ranks += 1;
            let key = node.branch.as_deref().unwrap_or("");
            let entry = branches.entry(key).or_insert_with(|| BranchCost {
                own: left.branch.get(key).copied().unwrap_or(0),
                ..BranchCost::default()
            });
            entry.planned += 1;
            if left.can_pay(node) {
                left.spend(node);
            } else {
                entry.missing += 1;
                cost.missing += 1;
                cost.affordable = cost.affordable.min(index);
            }
        }
        cost.branches = branches
            .into_iter()
            .map(|(id, branch)| (id.to_owned(), branch))
            .collect();
        cost.general_used = general_have - left.general.min(general_have);
        cost
    }
}

/// The ranks of `profile` as steps from the start, in an order the rules allow: a node comes
/// once a full neighbour is in. Switched-off nodes count as learned, as in the rules.
pub fn learned_steps(tree: &Tree, profile: &Profile) -> Vec<u16> {
    let mut sim = Sim::new(tree, &Ranks::from([(tree.start, 1)]));
    let mut todo: Vec<(u16, u8)> = profile
        .ranks
        .iter()
        .filter(|(id, rank)| **id != tree.start && **rank > 0 && tree.nodes.contains_key(id))
        .map(|(id, rank)| (*id, *rank))
        .collect();
    let mut steps = Vec::new();
    loop {
        let before = todo.len();
        todo.retain(|&(id, rank)| {
            if sim.fits(id).is_err() {
                return true;
            }
            for _ in 0..rank {
                if sim.fits(id).is_ok() {
                    sim.learn(id);
                    steps.push(id);
                }
            }
            false
        });
        if todo.len() == before {
            return steps;
        }
    }
}

/// Why a share string was refused, in words for the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareError {
    NotAShare,
    TooLong,
    OtherVersion(String),
    Damaged,
    OtherLayout { theirs: u32, ours: u16 },
    UnknownNode(u32),
    TooManyRanks(u16),
}

impl ShareError {
    pub fn message(&self) -> String {
        match self {
            ShareError::NotAShare => "Это не строка сборки".to_owned(),
            ShareError::TooLong => "Строка сборки слишком длинная".to_owned(),
            ShareError::OtherVersion(version) => {
                format!("Строка сборки версии {version}, а мод читает версию {SHARE_VERSION}")
            }
            ShareError::Damaged => "Строка сборки повреждена".to_owned(),
            ShareError::OtherLayout { theirs, ours } => {
                format!("Строка сборки для другой нумерации узлов ({theirs}, у дерева {ours})")
            }
            ShareError::UnknownNode(id) => format!("В строке сборки неизвестный узел {id}"),
            ShareError::TooManyRanks(id) => format!("В строке сборки лишние ранги узла {id}"),
        }
    }
}

fn push_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7F) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// A varint of at most five bytes; `None` when it runs off the end or past `u32`.
fn read_varint(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let mut value: u64 = 0;
    for shift in 0..5 {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7F) << (7 * shift);
        if byte & 0x80 == 0 {
            return u32::try_from(value).ok();
        }
    }
    None
}

/// FNV-1a folded to 16 bits: enough to catch a mistyped or cut string.
fn checksum(bytes: &[u8]) -> u16 {
    let hash = bytes.iter().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    });
    (hash ^ (hash >> 16)) as u16
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn to_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn from_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut count) = (0u32, 0u32);
    for c in text.bytes() {
        let value = ALPHABET.iter().position(|a| *a == c)? as u32;
        bits = bits << 6 | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    // Leftover bits are padding and must be zero, as `to_base64` writes them.
    (count < 6 && bits == 0).then_some(out)
}

/// The share string of `steps` under node numbering `layout`: `PCT1:` and URL-safe base64 of
/// the numbering, runs of ranks by node and a checksum.
pub fn encode(layout: u16, steps: &[u16]) -> String {
    let mut bytes = Vec::with_capacity(steps.len() * 2 + 4);
    push_varint(&mut bytes, u32::from(layout));
    let mut i = 0;
    while i < steps.len() {
        let id = steps[i];
        let mut run = 1u8;
        while run < RUN && steps.get(i + usize::from(run)) == Some(&id) {
            run += 1;
        }
        push_varint(&mut bytes, u32::from(id) << 2 | u32::from(run - 1));
        i += usize::from(run);
    }
    let sum = checksum(&bytes);
    bytes.extend(sum.to_le_bytes());
    format!("{SHARE_PREFIX}{SHARE_VERSION}:{}", to_base64(&bytes))
}

/// Whether `text` is meant as a share string, valid or not: the search box takes it as one.
pub fn looks_like_share(text: &str) -> bool {
    let text = text.trim();
    text.len() > SHARE_PREFIX.len() + 1
        && text.is_char_boundary(SHARE_PREFIX.len())
        && text[..SHARE_PREFIX.len()].eq_ignore_ascii_case(SHARE_PREFIX)
        && text[SHARE_PREFIX.len()..]
            .split_once(':')
            .is_some_and(|(version, _)| {
                !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit())
            })
}

/// Reads a share string against `tree` and its node numbering `layout`. Nothing in it is
/// trusted: the shape, the checksum, the numbering, every node id and every rank count are
/// checked, and the steps are fitted to a profile only afterwards (`Plan::fitted`).
pub fn decode(text: &str, tree: &Tree, layout: u16) -> Result<Vec<u16>, ShareError> {
    let text = text.trim();
    if text.len() > MAX_SHARE_LEN {
        return Err(ShareError::TooLong);
    }
    if !looks_like_share(text) {
        return Err(ShareError::NotAShare);
    }
    let (version, body) = text[SHARE_PREFIX.len()..]
        .split_once(':')
        .ok_or(ShareError::NotAShare)?;
    if version != SHARE_VERSION.to_string() {
        return Err(ShareError::OtherVersion(version.chars().take(6).collect()));
    }
    let bytes = from_base64(body).ok_or(ShareError::Damaged)?;
    let (payload, sum) = bytes.split_last_chunk::<2>().ok_or(ShareError::Damaged)?;
    if checksum(payload) != u16::from_le_bytes(*sum) {
        return Err(ShareError::Damaged);
    }
    let mut at = 0;
    let theirs = read_varint(payload, &mut at).ok_or(ShareError::Damaged)?;
    if theirs != u32::from(layout) {
        return Err(ShareError::OtherLayout {
            theirs,
            ours: layout,
        });
    }
    let mut steps = Vec::new();
    let mut ranks: BTreeMap<u16, u8> = BTreeMap::new();
    while at < payload.len() {
        let entry = read_varint(payload, &mut at).ok_or(ShareError::Damaged)?;
        let raw_id = entry >> 2;
        let run = (entry & 3) as u8 + 1;
        let id = u16::try_from(raw_id).map_err(|_| ShareError::UnknownNode(raw_id))?;
        let node = tree.nodes.get(&id).ok_or(ShareError::UnknownNode(raw_id))?;
        let rank = ranks.entry(id).or_insert(0);
        *rank = rank.saturating_add(run);
        if *rank > node.max_rank() || node.kind == Kind::Start {
            return Err(ShareError::TooManyRanks(id));
        }
        if steps.len() + usize::from(run) > MAX_STEPS {
            return Err(ShareError::TooLong);
        }
        steps.extend(std::iter::repeat_n(id, usize::from(run)));
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A start with two branches: a1 (two ranks) leads to a2; b1 leads to b2.
    const TREE: &str = r##"{
        "start": 0, "general_share": 0.5, "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 6},
        "branches": {"a": {"name": "A"}, "b": {"name": "B"}},
        "base_attributes": {},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "start", "name": "s", "kind": "start", "links": [1, 3, 5]},
            {"id": 1, "x": 0, "y": -100, "icon": "i", "name": "a1", "kind": "small", "branch": "a",
             "links": [2], "ranks": 2},
            {"id": 2, "x": 0, "y": -140, "icon": "i", "name": "a2", "kind": "big", "branch": "a"},
            {"id": 3, "x": 0, "y": 100, "icon": "i", "name": "b1", "kind": "small", "branch": "b",
             "links": [4]},
            {"id": 4, "x": 0, "y": 140, "icon": "i", "name": "b2", "kind": "small", "branch": "b"},
            {"id": 300, "x": 100, "y": 0, "icon": "i", "name": "free", "kind": "small"},
            {"id": 5, "x": 50, "y": 0, "icon": "i", "name": "c", "kind": "small", "links": [300]}
        ]
    }"##;

    fn tree() -> Tree {
        Tree::parse(TREE.as_bytes()).unwrap()
    }

    /// Experience for `levels` levels of a branch with coefficient 10: 10 + 20 + 30...
    fn xp(levels: u32) -> f64 {
        (1..=levels).map(|l| f64::from(l * 10)).sum()
    }

    fn rich(tree: &Tree, a: u32, b: u32, general: u32) -> Profile {
        let mut profile = tree.new_profile();
        profile.xp.insert("a".to_owned(), xp(a));
        profile.xp.insert("b".to_owned(), xp(b));
        profile.general_xp = xp(general);
        profile
    }

    #[test]
    fn clicks_build_a_path_only_through_reachable_ranks() {
        let tree = tree();
        let profile = tree.new_profile();
        let plan = Plan::default();
        assert!(
            plan.with(&tree, &profile, 2).is_none(),
            "a2 needs a1 in full"
        );
        let plan = plan.with(&tree, &profile, 1).unwrap();
        assert!(
            plan.with(&tree, &profile, 2).is_none(),
            "a1 is not full yet"
        );
        let plan = plan.with(&tree, &profile, 1).unwrap();
        assert!(plan.with(&tree, &profile, 1).is_none(), "a1 has two ranks");
        let plan = plan.with(&tree, &profile, 2).unwrap();
        assert_eq!(plan.steps(), [1, 1, 2]);
        assert!(
            plan.with(&tree, &profile, 0).is_none(),
            "the start is never planned"
        );
        assert!(plan.with(&tree, &profile, 77).is_none());
        assert_eq!(plan.planned(), BTreeMap::from([(1, 2), (2, 1)]));
        assert_eq!(plan.parents(&tree, &profile), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn removing_a_node_drops_what_only_it_led_to() {
        let tree = tree();
        let profile = tree.new_profile();
        let mut plan = Plan::default();
        for id in [1, 1, 2, 3, 4] {
            plan = plan.with(&tree, &profile, id).unwrap();
        }
        assert_eq!(plan.without(&tree, &profile, 1).steps(), [3, 4]);
        assert_eq!(plan.without(&tree, &profile, 4).steps(), [1, 1, 2, 3]);
    }

    #[test]
    fn a_new_profile_takes_the_learned_ranks_out_of_the_plan() {
        let tree = tree();
        let profile = rich(&tree, 3, 0, 0);
        let mut plan = Plan::default();
        for id in [1, 1, 2] {
            plan = plan.with(&tree, &profile, id).unwrap();
        }
        let once = tree.allocate(&profile, 1).unwrap();
        assert_eq!(plan.refit(&tree, &once).steps(), [1, 2]);
        let twice = tree.allocate(&once, 1).unwrap();
        assert_eq!(plan.refit(&tree, &twice).steps(), [2]);
        assert_eq!(plan.refit(&tree, &tree.respec(&twice)).steps(), [1, 1, 2]);
    }

    #[test]
    fn the_cost_pays_branch_points_first_then_general_ones() {
        let tree = tree();
        // Two points of a, none of b, one general.
        let profile = rich(&tree, 2, 0, 1);
        let mut plan = Plan::default();
        for id in [1, 1, 2, 3, 4] {
            plan = plan.with(&tree, &profile, id).unwrap();
        }
        let cost = plan.cost(&tree, &profile);
        assert_eq!(cost.ranks, 5);
        assert_eq!(
            cost.branches["a"],
            BranchCost {
                planned: 3,
                own: 2,
                missing: 0
            }
        );
        assert_eq!(
            cost.branches["b"],
            BranchCost {
                planned: 2,
                own: 0,
                missing: 2
            }
        );
        assert_eq!((cost.general_used, cost.general_have), (1, 1));
        assert_eq!(cost.missing, 2);
        assert_eq!(cost.affordable, 3);
        // The same as allocating step by step.
        let mut grown = profile.clone();
        for id in &plan.steps()[..cost.affordable] {
            grown = tree.allocate(&grown, *id).unwrap();
        }
        assert!(
            tree.allocate(&grown, plan.steps()[cost.affordable])
                .is_err()
        );
    }

    #[test]
    fn nodes_outside_branches_pay_with_general_points() {
        let tree = tree();
        let profile = rich(&tree, 0, 0, 1);
        let plan = Plan::default()
            .with(&tree, &profile, 5)
            .and_then(|p| p.with(&tree, &profile, 300))
            .unwrap();
        let cost = plan.cost(&tree, &profile);
        assert_eq!(cost.branches[""].planned, 2);
        assert_eq!(
            (cost.missing, cost.affordable, cost.general_used),
            (1, 1, 1)
        );
    }

    #[test]
    fn share_strings_round_trip_with_runs_and_big_ids() {
        let tree = tree();
        let steps = vec![1, 1, 2, 5, 300, 3, 4];
        let text = encode(2, &steps);
        assert!(text.starts_with("PCT1:"), "{text}");
        assert!(looks_like_share(&text));
        assert!(text.len() < 30, "{text}");
        assert_eq!(decode(&text, &tree, 2), Ok(steps));
        assert_eq!(
            decode(&format!("  {text}\n"), &tree, 2).map(|s| s.len()),
            Ok(7)
        );
        assert_eq!(decode(&encode(2, &[]), &tree, 2), Ok(vec![]));
    }

    #[test]
    fn share_strings_are_never_trusted() {
        let tree = tree();
        let good = encode(2, &[1, 1, 2]);
        assert_eq!(decode("hello", &tree, 2), Err(ShareError::NotAShare));
        assert_eq!(decode("PCT:abc", &tree, 2), Err(ShareError::NotAShare));
        assert_eq!(
            decode(&good.replacen("PCT1", "PCT7", 1), &tree, 2),
            Err(ShareError::OtherVersion("7".to_owned()))
        );
        assert!(matches!(
            decode(&good, &tree, 3),
            Err(ShareError::OtherLayout { theirs: 2, ours: 3 })
        ));
        // A changed character breaks the checksum or the base64.
        let mut bytes = good.clone().into_bytes();
        let last = bytes.len() - 3;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let broken = String::from_utf8(bytes).unwrap();
        assert_eq!(decode(&broken, &tree, 2), Err(ShareError::Damaged));
        assert_eq!(decode("PCT1:@@@@", &tree, 2), Err(ShareError::Damaged));
        assert_eq!(decode("PCT1:", &tree, 2), Err(ShareError::Damaged));
        assert_eq!(decode("PCT1:AA", &tree, 2), Err(ShareError::Damaged));
        assert_eq!(
            decode(&encode(2, &[1, 999]), &tree, 2),
            Err(ShareError::UnknownNode(999))
        );
        assert_eq!(
            decode(&encode(2, &[2, 2]), &tree, 2),
            Err(ShareError::TooManyRanks(2))
        );
        assert_eq!(
            decode(&encode(2, &[0]), &tree, 2),
            Err(ShareError::TooManyRanks(0))
        );
        let huge = format!("PCT1:{}", "A".repeat(MAX_SHARE_LEN));
        assert_eq!(decode(&huge, &tree, 2), Err(ShareError::TooLong));
        assert!(!ShareError::Damaged.message().is_empty());
    }

    #[test]
    fn imported_steps_are_fitted_to_the_profile_they_land_on() {
        let tree = tree();
        let theirs = rich(&tree, 6, 6, 6);
        let mut built = theirs.clone();
        for id in [1, 1, 2, 3] {
            built = tree.allocate(&built, id).unwrap();
        }
        let steps = learned_steps(&tree, &built);
        assert_eq!(
            Plan::fitted(&tree, &tree.new_profile(), &steps)
                .0
                .steps()
                .len(),
            4
        );
        let mine = tree.allocate(&theirs, 3).unwrap();
        let (plan, skipped) = Plan::fitted(&tree, &mine, &steps);
        assert_eq!(plan.steps(), [1, 1, 2]);
        assert_eq!(
            skipped,
            Skipped {
                learned: 1,
                unreachable: 0
            }
        );
        let (plan, skipped) = Plan::fitted(&tree, &tree.new_profile(), &[2, 1]);
        assert_eq!((plan.steps(), skipped.unreachable), (&[1][..], 1));
    }

    #[test]
    fn learned_steps_replay_into_the_same_ranks() {
        let tree = tree();
        let mut profile = rich(&tree, 6, 6, 6);
        for id in [3, 1, 4, 1, 2, 5, 300] {
            profile = tree.allocate(&profile, id).unwrap();
        }
        let steps = learned_steps(&tree, &profile);
        let mut replayed = rich(&tree, 6, 6, 6);
        for id in &steps {
            replayed = tree.allocate(&replayed, *id).unwrap();
        }
        assert_eq!(replayed.ranks, profile.ranks);
    }
}
