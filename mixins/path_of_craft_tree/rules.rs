// The tree's data, progression and message formats, free of game types so `cargo test` covers
// them in seconds.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Bumped whenever the messages between the client and server halves change.
pub const PROTOCOL: u16 = 5;
/// The node numbering saved profiles refer to. A profile saved under another one keeps its
/// experience and loses its nodes: the ids mean different nodes now.
pub const PROFILE_LAYOUT: u16 = 2;
/// Payload of one message part; mod messages are capped at 30 000 bytes.
pub const MAX_PART: usize = 28_000;
/// Minecraft levels that resetting the tree costs.
pub const RESPEC_LEVELS: i32 = 5;
const LEVEL_BONUS: f64 = 0.002;

const PART_HEADER: usize = 4;

pub type Allocated = BTreeSet<u16>;
/// The rank of every learned node: 1 up to the node's `ranks`.
pub type Ranks = BTreeMap<u16, u8>;
pub type Effects = BTreeMap<String, f64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Start,
    Small,
    Big,
    Upgrade,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Bonus {
    pub attribute: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Node {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    pub icon: String,
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub links: Vec<u16>,
    #[serde(default)]
    pub bonuses: Vec<Bonus>,
    /// Effects of one rank; a node with several ranks gives them once per rank.
    #[serde(default)]
    pub effects: Effects,
    /// How many times the node can be learned, each time for one point.
    #[serde(default = "one_rank")]
    pub ranks: u8,
    /// What the node gives at each rank, for the tooltip.
    #[serde(default)]
    pub rank_descriptions: Vec<String>,
    /// The node is in the tree but its effect is not implemented yet.
    #[serde(default)]
    pub todo: bool,
}

fn one_rank() -> u8 {
    1
}

impl Node {
    /// The highest rank, never below 1.
    pub fn max_rank(&self) -> u8 {
        self.ranks.max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Curve {
    pub coefficient: f64,
    pub exponent: f64,
    pub max_level: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Branch {
    pub name: String,
    /// Only a tool whose id ends with this earns the branch's experience.
    #[serde(default)]
    pub tool_suffix: Option<String>,
    /// Experience by loot table (`minecraft:blocks/diamond_ore`).
    #[serde(default)]
    pub xp: BTreeMap<String, f64>,
    /// The branch's colour on the colour wheel, `#rrggbb`.
    #[serde(default)]
    pub colour: Option<String>,
    /// The emblem's icon id.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Ore {
    /// The raw item this ore drops, and the ingot it smelts into.
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub ingot: Option<String>,
    /// The item that gets extra drops; greed only applies to rich ores.
    pub drop: String,
    #[serde(default)]
    pub rich: bool,
}

/// A related action: experience for a second branch, as for `xp` of its own work.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SmeltCross {
    pub branch: String,
    pub xp: f64,
}

#[derive(Deserialize)]
struct RawTree {
    start: u16,
    general_share: f64,
    cross_share: f64,
    /// The share of a hand action's experience a player's machine earns them.
    #[serde(default = "default_machine_share")]
    machine_share: f64,
    curve: Curve,
    branches: BTreeMap<String, Branch>,
    #[serde(default)]
    ores: BTreeMap<String, Ore>,
    #[serde(default)]
    smelt_cross: Option<SmeltCross>,
    #[serde(default)]
    base_attributes: BTreeMap<String, f64>,
    #[serde(default)]
    level_bonus: BTreeMap<String, String>,
    /// Named sets of ids the experience rules ask about (foods, ingots, bosses...).
    #[serde(default)]
    lists: BTreeMap<String, BTreeSet<String>>,
    /// Numbers by id: a mob's health, a crop's ripe age.
    #[serde(default)]
    numbers: BTreeMap<String, BTreeMap<String, f64>>,
    nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub start: u16,
    pub general_share: f64,
    pub cross_share: f64,
    pub machine_share: f64,
    pub curve: Curve,
    pub branches: BTreeMap<String, Branch>,
    pub ores: BTreeMap<String, Ore>,
    pub smelt_cross: Option<SmeltCross>,
    pub base_attributes: BTreeMap<String, f64>,
    pub lists: BTreeMap<String, BTreeSet<String>>,
    pub numbers: BTreeMap<String, BTreeMap<String, f64>>,
    pub nodes: BTreeMap<u16, Node>,
    /// For each branch, the effect key that grows with its level.
    pub level_bonus: BTreeMap<String, String>,
    /// The total experience at the end of each level, from the first: levels are looked up here.
    level_ends: Vec<f64>,
}

/// The total experience each level of `curve` ends at. The steps are whole numbers, so the sums
/// are exact and match subtracting them one by one.
fn level_ends(curve: &Curve) -> Vec<f64> {
    (1..=curve.max_level)
        .scan(0.0, |total, level| {
            *total += (curve.coefficient * f64::from(level).powf(curve.exponent)).floor();
            Some(*total)
        })
        .collect()
}

/// Adds experience to a branch; the branch name is copied only the first time it earns any.
fn add_xp(xp: &mut BTreeMap<String, f64>, branch: &str, amount: f64) {
    match xp.get_mut(branch) {
        Some(total) => *total += amount,
        None => {
            xp.insert(branch.to_owned(), amount);
        }
    }
}

/// Points a player has left: each branch's own, and general ones.
#[derive(Debug, Clone, PartialEq)]
pub struct Points<'a> {
    pub branch: BTreeMap<&'a str, u32>,
    pub general: u32,
}

impl Points<'_> {
    /// Whether the next rank of `node` can be paid for: its branch's points first, then general.
    pub fn can_pay(&self, node: &Node) -> bool {
        let own = node
            .branch
            .as_deref()
            .and_then(|b| self.branch.get(b))
            .is_some_and(|points| *points > 0);
        own || self.general > 0
    }

    /// Pays for one more rank of `node` the way a recount would: its branch's points while they
    /// last, general points after that; nodes outside every branch pay with general points.
    pub fn spend(&mut self, node: &Node) {
        if node.kind == Kind::Start {
            return;
        }
        let own = match node.branch.as_deref() {
            Some(branch) => match self.branch.get_mut(branch) {
                Some(points) => points,
                None => return,
            },
            None => &mut self.general,
        };
        if *own > 0 {
            *own -= 1;
        } else {
            self.general = self.general.saturating_sub(1);
        }
    }
}

/// What a player has earned and spent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// The node numbering `ranks` refers to; 0 for files older than ranks.
    #[serde(default)]
    pub layout: u16,
    #[serde(default)]
    pub ranks: Ranks,
    #[serde(default)]
    pub xp: BTreeMap<String, f64>,
    #[serde(default)]
    pub general_xp: f64,
    /// One-time rewards already paid: biomes, dimensions, first crafts.
    #[serde(default)]
    pub seen: BTreeSet<String>,
    /// Big nodes the player switched off; their upgrades stop working with them.
    #[serde(default)]
    pub disabled: Allocated,
}

/// What the client is sent of a profile: everything but the one-time rewards, which only the
/// server checks. It reads back as a `Profile` with an empty `seen`.
#[derive(Serialize)]
pub struct ClientProfile<'a> {
    layout: u16,
    ranks: &'a Ranks,
    xp: &'a BTreeMap<String, f64>,
    general_xp: f64,
    disabled: &'a Allocated,
}

impl Profile {
    /// The part of the profile a client needs, for its state message.
    pub fn for_client(&self) -> ClientProfile<'_> {
        ClientProfile {
            layout: self.layout,
            ranks: &self.ranks,
            xp: &self.xp,
            general_xp: self.general_xp,
            disabled: &self.disabled,
        }
    }

    /// The rank of node `id`, 0 when it is not learned.
    pub fn rank(&self, id: u16) -> u8 {
        self.ranks.get(&id).copied().unwrap_or(0)
    }

    pub fn is_learned(&self, id: u16) -> bool {
        self.rank(id) > 0
    }

    /// Reads a saved profile. One saved under another node numbering keeps its experience and
    /// one-time rewards; its nodes are dropped. The second value is true when that happened.
    pub fn from_saved(bytes: &[u8]) -> Result<(Profile, bool), String> {
        let saved: Profile = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if saved.layout == PROFILE_LAYOUT {
            return Ok((saved, false));
        }
        let kept = Profile {
            layout: PROFILE_LAYOUT,
            ranks: Ranks::new(),
            disabled: Allocated::new(),
            ..saved
        };
        Ok((kept, true))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    UnknownNode,
    AlreadyAllocated,
    NotLinked,
    NoPointsLeft,
}

fn collect_nodes(raw_nodes: Vec<Node>) -> Result<BTreeMap<u16, Node>, String> {
    let mut nodes = BTreeMap::new();
    for node in raw_nodes {
        let id = node.id;
        if nodes.insert(id, node).is_some() {
            return Err(format!("node {id} is defined twice"));
        }
    }
    Ok(nodes)
}

fn check_node(node: &Node, raw: &RawTree, nodes: &BTreeMap<u16, Node>) -> Result<(), String> {
    if !node.x.is_finite() || !node.y.is_finite() {
        return Err(format!(
            "node {} has a position that is not a number",
            node.id
        ));
    }
    if let Some(branch) = &node.branch
        && !raw.branches.contains_key(branch)
    {
        return Err(format!("node {} uses unknown branch {branch}", node.id));
    }
    for link in &node.links {
        if *link == node.id {
            return Err(format!("node {} links to itself", node.id));
        }
        if !nodes.contains_key(link) {
            return Err(format!("node {} links to unknown node {link}", node.id));
        }
    }
    if node.effects.values().any(|value| !value.is_finite()) {
        return Err(format!(
            "node {} has an effect that is not a number",
            node.id
        ));
    }
    for bonus in &node.bonuses {
        if !bonus.amount.is_finite() {
            return Err(format!("node {} has a bonus that is not a number", node.id));
        }
        if !raw.base_attributes.contains_key(&bonus.attribute) {
            return Err(format!(
                "node {} uses {} which base_attributes does not list",
                node.id, bonus.attribute
            ));
        }
    }
    Ok(())
}

fn with_symmetric_links(nodes: BTreeMap<u16, Node>) -> BTreeMap<u16, Node> {
    let pairs: Vec<(u16, u16)> = nodes
        .values()
        .flat_map(|node| node.links.iter().map(|link| (*link, node.id)))
        .collect();
    let mut linked = nodes;
    for (from, to) in pairs {
        if let Some(node) = linked.get_mut(&from)
            && !node.links.contains(&to)
        {
            node.links.push(to);
        }
    }
    linked
}

fn default_machine_share() -> f64 {
    0.5
}

fn check_tree(raw: &RawTree) -> Result<(), String> {
    if !(0.0..=1.0).contains(&raw.machine_share) {
        return Err("machine_share must be between 0 and 1".to_owned());
    }
    if raw.curve.coefficient <= 0.0 || raw.curve.exponent <= 0.0 || raw.curve.max_level == 0 {
        return Err("curve must have a positive coefficient, exponent and max_level".to_owned());
    }
    if !(0.0..=1.0).contains(&raw.general_share) || !(0.0..=1.0).contains(&raw.cross_share) {
        return Err("general_share and cross_share must be between 0 and 1".to_owned());
    }
    if let Some(cross) = &raw.smelt_cross
        && !raw.branches.contains_key(&cross.branch)
    {
        return Err(format!("smelt_cross uses unknown branch {}", cross.branch));
    }
    let values = raw.branches.values().flat_map(|branch| branch.xp.values());
    if values
        .chain(raw.base_attributes.values())
        .any(|value| !value.is_finite())
    {
        return Err("an experience or attribute value is not a number".to_owned());
    }
    Ok(())
}

impl Tree {
    /// Reads and checks a tree. A link written on one side counts for both.
    pub fn parse(json: &[u8]) -> Result<Tree, String> {
        let mut raw: RawTree = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        check_tree(&raw)?;
        let nodes = collect_nodes(std::mem::take(&mut raw.nodes))?;
        if !nodes.contains_key(&raw.start) {
            return Err(format!("start node {} does not exist", raw.start));
        }
        for node in nodes.values() {
            check_node(node, &raw, &nodes)?;
        }
        let level_ends = level_ends(&raw.curve);
        Ok(Tree {
            start: raw.start,
            general_share: raw.general_share,
            cross_share: raw.cross_share,
            machine_share: raw.machine_share,
            curve: raw.curve,
            branches: raw.branches,
            ores: raw.ores,
            smelt_cross: raw.smelt_cross,
            base_attributes: raw.base_attributes,
            level_bonus: raw.level_bonus,
            lists: raw.lists,
            numbers: raw.numbers,
            nodes: with_symmetric_links(nodes),
            level_ends,
        })
    }

    /// What a new player has: the start node, which costs nothing.
    pub fn new_profile(&self) -> Profile {
        Profile {
            layout: PROFILE_LAYOUT,
            ranks: self.start_ranks(),
            ..Profile::default()
        }
    }

    fn start_ranks(&self) -> Ranks {
        Ranks::from([(self.start, 1)])
    }

    /// The experience one level needs, from the level below it.
    pub fn xp_for_level(&self, level: u32) -> f64 {
        (self.curve.coefficient * f64::from(level).powf(self.curve.exponent)).floor()
    }

    /// The level `xp` reaches, and how far it is into the next one (0 to 1).
    pub fn level_progress(&self, xp: f64) -> (u32, f64) {
        let reached = self.level_ends.partition_point(|end| *end <= xp);
        let Some(end) = self.level_ends.get(reached) else {
            return (self.curve.max_level, 1.0);
        };
        let start = reached
            .checked_sub(1)
            .map_or(0.0, |below| self.level_ends[below]);
        // Bounded by max_level, which is a u32.
        let level = reached as u32;
        (level, (xp - start) / (end - start))
    }

    pub fn branch_level(&self, profile: &Profile, branch: &str) -> u32 {
        self.level_progress(profile.xp.get(branch).copied().unwrap_or(0.0))
            .0
    }

    /// Whether any branch moved to another level between two experience tables; the level bonus
    /// depends on it.
    pub fn levels_changed(
        &self,
        before: &BTreeMap<String, f64>,
        after: &BTreeMap<String, f64>,
    ) -> bool {
        after.iter().any(|(branch, &xp)| {
            let old = before.get(branch).copied().unwrap_or(0.0);
            old != xp && self.level_progress(old).0 != self.level_progress(xp).0
        })
    }

    pub fn general_level(&self, profile: &Profile) -> u32 {
        self.level_progress(profile.general_xp).0
    }

    /// Points spent on the nodes `keep` accepts: one for each rank.
    fn spent_on(&self, ranks: &Ranks, keep: impl Fn(&Node) -> bool) -> u32 {
        ranks
            .iter()
            .filter(|(id, _)| self.nodes.get(id).is_some_and(&keep))
            .map(|(_, rank)| u32::from(*rank))
            .sum()
    }

    fn spent_in(&self, ranks: &Ranks, branch: &str) -> u32 {
        self.spent_on(ranks, |node| node.branch.as_deref() == Some(branch))
    }

    /// Points left for `branch` of its own.
    pub fn branch_points(&self, profile: &Profile, branch: &str) -> u32 {
        self.branch_level(profile, branch)
            .saturating_sub(self.spent_in(&profile.ranks, branch))
    }

    /// General points left. Branch points pay first; what they do not cover uses general points.
    pub fn general_points(&self, profile: &Profile) -> u32 {
        self.points(profile).general
    }

    /// Points left for every branch and general ones, counted in one pass over the ranks.
    pub fn points(&self, profile: &Profile) -> Points<'_> {
        let mut spent: BTreeMap<&str, u32> =
            self.branches.keys().map(|b| (b.as_str(), 0)).collect();
        let mut unbranched = 0;
        for (id, rank) in &profile.ranks {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            match node.branch.as_deref() {
                Some(branch) => {
                    if let Some(sum) = spent.get_mut(branch) {
                        *sum += u32::from(*rank);
                    }
                }
                None if node.kind != Kind::Start => unbranched += u32::from(*rank),
                None => {}
            }
        }
        let mut overflow = 0;
        let branch = spent
            .into_iter()
            .map(|(id, spent)| {
                let level = self.branch_level(profile, id);
                overflow += spent.saturating_sub(level);
                (id, level.saturating_sub(spent))
            })
            .collect();
        let general = self
            .general_level(profile)
            .saturating_sub(overflow)
            .saturating_sub(unbranched);
        Points { branch, general }
    }

    /// A branch's own points pay first, so the full count is only needed when they run out.
    fn can_pay(&self, profile: &Profile, node: &Node) -> bool {
        let own = node
            .branch
            .as_ref()
            .is_some_and(|branch| self.branch_points(profile, branch) > 0);
        own || self.general_points(profile) > 0
    }

    /// Whether node `id` has all its ranks.
    pub fn is_full(&self, profile: &Profile, id: u16) -> bool {
        self.nodes
            .get(&id)
            .is_some_and(|node| profile.rank(id) >= node.max_rank())
    }

    /// Whether a neighbour of `id` has all its ranks: only a full node opens the way on.
    pub fn is_reachable(&self, profile: &Profile, id: u16) -> bool {
        self.nodes
            .get(&id)
            .is_some_and(|node| node.links.iter().any(|link| self.is_full(profile, *link)))
    }

    /// Whether the next rank of `id` can be learned now.
    pub fn can_allocate(&self, profile: &Profile, id: u16) -> Result<(), Refusal> {
        self.check_allocate(profile, id, |node| self.can_pay(profile, node))
    }

    /// `can_allocate` with the payment check supplied, for callers that keep points as they go.
    fn check_allocate(
        &self,
        profile: &Profile,
        id: u16,
        can_pay: impl FnOnce(&Node) -> bool,
    ) -> Result<(), Refusal> {
        let Some(node) = self.nodes.get(&id) else {
            return Err(Refusal::UnknownNode);
        };
        if profile.rank(id) >= node.max_rank() {
            return Err(Refusal::AlreadyAllocated);
        }
        if !self.is_reachable(profile, id) {
            return Err(Refusal::NotLinked);
        }
        if !can_pay(node) {
            return Err(Refusal::NoPointsLeft);
        }
        Ok(())
    }

    /// The profile after learning one more rank of `id`. The server calls this on every request
    /// from a client.
    pub fn allocate(&self, profile: &Profile, id: u16) -> Result<Profile, Refusal> {
        self.can_allocate(profile, id)?;
        let mut next = profile.clone();
        next.ranks.insert(id, profile.rank(id) + 1);
        Ok(next)
    }

    /// Experience for the branch doing the work; a share also goes to the general scale.
    pub fn award_main(&self, profile: &Profile, branch: &str, amount: f64) -> Profile {
        let mut next = profile.clone();
        self.add_main_xp(&mut next, branch, amount);
        next
    }

    /// Experience for a related branch: reduced, and it does not feed the general scale.
    pub fn award_cross(&self, profile: &Profile, branch: &str, amount: f64) -> Profile {
        let mut next = profile.clone();
        self.add_cross_xp(&mut next, branch, amount);
        next
    }

    /// `award_main` on the profile itself, without copying it.
    pub fn add_main_xp(&self, profile: &mut Profile, branch: &str, amount: f64) {
        add_xp(&mut profile.xp, branch, amount);
        profile.general_xp += amount * self.general_share;
    }

    /// `award_cross` on the profile itself, without copying it.
    pub fn add_cross_xp(&self, profile: &mut Profile, branch: &str, amount: f64) {
        add_xp(&mut profile.xp, branch, amount * self.cross_share);
    }

    /// The branch and experience for a loot table broken with `tool`.
    pub fn xp_for_break(&self, table: &str, tool: &str) -> Option<(&str, f64)> {
        self.branches.iter().find_map(|(id, branch)| {
            let fits = branch
                .tool_suffix
                .as_ref()
                .is_none_or(|suffix| tool.ends_with(suffix.as_str()));
            branch
                .xp
                .get(table)
                .filter(|_| fits)
                .map(|xp| (id.as_str(), *xp))
        })
    }

    /// The branch and experience for an action such as `act:fish`.
    pub fn xp_for_action(&self, action: &str) -> Option<(&str, f64)> {
        self.branches
            .iter()
            .find_map(|(id, branch)| branch.xp.get(action).map(|xp| (id.as_str(), *xp)))
    }

    pub fn in_list(&self, list: &str, id: &str) -> bool {
        self.lists.get(list).is_some_and(|ids| ids.contains(id))
    }

    /// Whether `id` ends with one of the list's suffixes.
    pub fn has_suffix_in(&self, list: &str, id: &str) -> bool {
        self.lists
            .get(list)
            .is_some_and(|suffixes| suffixes.iter().any(|s| id.ends_with(s.as_str())))
    }

    pub fn number(&self, table: &str, id: &str) -> Option<f64> {
        self.numbers.get(table)?.get(id).copied()
    }

    /// Whether any branch pays experience for breaking this loot table.
    pub fn earns_xp(&self, table: &str) -> bool {
        self.branches
            .values()
            .any(|branch| branch.xp.contains_key(table))
    }

    /// Every attribute's base value with the learned bonuses added, once per rank.
    pub fn attributes(&self, ranks: &Ranks) -> BTreeMap<String, f64> {
        let mut values = self.base_attributes.clone();
        let learned = ranks
            .iter()
            .filter_map(|(id, rank)| self.nodes.get(id).map(|node| (node, f64::from(*rank))));
        for (node, times) in learned {
            for bonus in &node.bonuses {
                if let Some(value) = values.get_mut(&bonus.attribute) {
                    *value += bonus.amount * times;
                }
            }
        }
        values
    }

    /// The big nodes switched off and every upgrade behind them.
    pub fn switched_off(&self, profile: &Profile) -> Allocated {
        let mut off = profile.disabled.clone();
        let mut frontier: Vec<u16> = profile.disabled.iter().copied().collect();
        while let Some(id) = frontier.pop() {
            let links = self.nodes.get(&id).map(|node| node.links.as_slice());
            for link in links.unwrap_or_default() {
                let upgrade = self
                    .nodes
                    .get(link)
                    .is_some_and(|n| n.kind == Kind::Upgrade);
                if upgrade && off.insert(*link) {
                    frontier.push(*link);
                }
            }
        }
        off
    }

    /// The summed effects of the player's working nodes, with the bonus of each branch level.
    pub fn effects(&self, profile: &Profile) -> Effects {
        let off = self.switched_off(profile);
        let mut sums = Effects::new();
        let working = profile
            .ranks
            .iter()
            .filter(|(id, _)| !off.contains(id))
            .filter_map(|(id, rank)| self.nodes.get(id).map(|node| (node, f64::from(*rank))))
            .filter(|(node, _)| !node.todo);
        for (node, times) in working {
            for (key, value) in &node.effects {
                *sums.entry(key.clone()).or_insert(0.0) += value * times;
            }
        }
        for (branch, key) in &self.level_bonus {
            let xp = profile.xp.get(branch).copied().unwrap_or(0.0);
            let (level, _) = self.level_progress(xp);
            *sums.entry(key.clone()).or_insert(0.0) += f64::from(level) * LEVEL_BONUS;
        }
        sums
    }

    /// Switches an allocated big node on or off; `None` for anything else.
    pub fn toggle(&self, profile: &Profile, id: u16) -> Option<Profile> {
        let big = self
            .nodes
            .get(&id)
            .is_some_and(|node| node.kind == Kind::Big);
        if !big || !profile.is_learned(id) {
            return None;
        }
        let mut next = profile.clone();
        if !next.disabled.remove(&id) {
            next.disabled.insert(id);
        }
        Some(next)
    }

    /// The profile with every point given back; experience is kept.
    pub fn respec(&self, profile: &Profile) -> Profile {
        Profile {
            ranks: self.start_ranks(),
            disabled: Allocated::new(),
            ..profile.clone()
        }
    }

    /// What survives of a saved profile after the tree changed: ranks of nodes that still exist,
    /// are connected to the start through full nodes and can be paid for, nearest first.
    pub fn sanitize(&self, saved: &Profile) -> Profile {
        let mut kept = Profile {
            layout: PROFILE_LAYOUT,
            ranks: self.start_ranks(),
            xp: saved.xp.clone(),
            general_xp: saved.general_xp,
            seen: saved.seen.clone(),
            disabled: Allocated::new(),
        };
        // Kept up to date rank by rank, instead of recounting every rank before each one.
        let mut points = self.points(&kept);
        let mut frontier = std::collections::VecDeque::from([self.start]);
        while let Some(id) = frontier.pop_front() {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            let mut links = node.links.clone();
            links.sort_unstable();
            for link in links {
                let wanted = saved.rank(link);
                let before = kept.rank(link);
                while kept.rank(link) < wanted
                    && self
                        .check_allocate(&kept, link, |node| points.can_pay(node))
                        .is_ok()
                {
                    kept.ranks.insert(link, kept.rank(link) + 1);
                    points.spend(&self.nodes[&link]);
                }
                if before == 0 && wanted > 0 && self.is_full(&kept, link) {
                    frontier.push_back(link);
                }
            }
        }
        kept.disabled = saved
            .disabled
            .iter()
            .filter(|id| kept.is_learned(**id))
            .copied()
            .collect();
        kept
    }

    /// Reads a saved profile and fits it to this tree; true when an old file lost its nodes.
    pub fn load_saved(&self, bytes: &[u8]) -> Result<(Profile, bool), String> {
        let (profile, dropped) = Profile::from_saved(bytes)?;
        Ok((self.sanitize(&profile), dropped))
    }
}

pub fn decode_u16(data: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(<[u8; 2]>::try_from(data).ok()?))
}

/// Splits `bytes` into messages small enough for the mod message cap.
pub fn split_parts(bytes: &[u8]) -> Vec<Vec<u8>> {
    let chunks: Vec<&[u8]> = bytes.chunks(MAX_PART).collect();
    let total = u16::try_from(chunks.len().max(1)).unwrap_or(u16::MAX);
    if chunks.is_empty() {
        return vec![[0u16.to_le_bytes(), total.to_le_bytes()].concat()];
    }
    chunks
        .iter()
        .zip(0u16..)
        .map(|(chunk, index)| [&index.to_le_bytes()[..], &total.to_le_bytes()[..], chunk].concat())
        .collect()
}

/// Collects the parts of one transfer; part 0 starts a new one.
#[derive(Debug, Default)]
pub struct Assembler {
    parts: Vec<Option<Vec<u8>>>,
}

impl Assembler {
    /// The whole payload once its last missing part arrives.
    pub fn push(&mut self, message: &[u8]) -> Option<Vec<u8>> {
        if message.len() < PART_HEADER {
            return None;
        }
        let index = usize::from(u16::from_le_bytes([message[0], message[1]]));
        let total = usize::from(u16::from_le_bytes([message[2], message[3]]));
        if index >= total {
            return None;
        }
        if index == 0 || self.parts.len() != total {
            self.parts = vec![None; total];
        }
        self.parts[index] = Some(message[PART_HEADER..].to_vec());
        if self.parts.iter().any(Option::is_none) {
            return None;
        }
        let whole = self.parts.iter().flatten().flatten().copied().collect();
        self.parts.clear();
        Some(whole)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every listed node at rank 1.
    fn learned(ids: &[u16]) -> Ranks {
        ids.iter().map(|id| (*id, 1)).collect()
    }

    #[test]
    fn switched_off_big_node_stops_its_upgrades_too() {
        let tree = Tree::parse(UPGRADE_TREE.as_bytes()).unwrap();
        let all = Profile {
            ranks: learned(&[0, 1, 2]),
            ..Profile::default()
        };
        assert_eq!(tree.effects(&all).get("k"), Some(&3.0));
        let off = tree.toggle(&all, 1).unwrap();
        assert_eq!(tree.effects(&off).get("k"), None);
        let on = tree.toggle(&off, 1).unwrap();
        assert_eq!(tree.effects(&on).get("k"), Some(&3.0));
    }

    #[test]
    fn only_learned_big_nodes_can_be_switched() {
        let tree = Tree::parse(UPGRADE_TREE.as_bytes()).unwrap();
        let start_only = Profile {
            ranks: learned(&[0]),
            ..Profile::default()
        };
        assert!(tree.toggle(&start_only, 1).is_none());
        let all = Profile {
            ranks: learned(&[0, 1, 2]),
            ..Profile::default()
        };
        assert!(tree.toggle(&all, 2).is_none());
    }

    #[test]
    fn respec_returns_points_and_keeps_experience() {
        let tree = Tree::parse(UPGRADE_TREE.as_bytes()).unwrap();
        let spent = Profile {
            ranks: learned(&[0, 1, 2]),
            disabled: Allocated::from([1]),
            xp: BTreeMap::from([("mining".to_owned(), 50.0)]),
            ..Profile::default()
        };
        let reset = tree.respec(&spent);
        assert_eq!(reset.ranks, learned(&[0]));
        assert!(reset.disabled.is_empty());
        assert_eq!(reset.xp, spent.xp);
    }

    #[test]
    fn machines_earn_half_unless_the_tree_says_otherwise() {
        assert_eq!(Tree::parse(TREE.as_bytes()).unwrap().machine_share, 0.5);
        let with_share = TREE.replacen("\"start\": 0,", "\"start\": 0, \"machine_share\": 0.3,", 1);
        assert_eq!(
            Tree::parse(with_share.as_bytes()).unwrap().machine_share,
            0.3
        );
        let too_much = TREE.replacen("\"start\": 0,", "\"start\": 0, \"machine_share\": 1.5,", 1);
        assert!(Tree::parse(too_much.as_bytes()).is_err());
    }

    #[test]
    fn level_bonus_grows_with_the_branch_level() {
        let tree = Tree::parse(UPGRADE_TREE.as_bytes()).unwrap();
        let profile = Profile {
            ranks: learned(&[0]),
            xp: BTreeMap::from([("mining".to_owned(), 10_000.0)]),
            ..Profile::default()
        };
        assert_eq!(
            tree.effects(&profile).get("speed"),
            Some(&(3.0 * LEVEL_BONUS))
        );
    }

    #[test]
    fn a_level_gained_by_any_branch_counts_as_a_level_change() {
        let tree = Tree::parse(TREE.as_bytes()).unwrap();
        let before = Profile {
            xp: BTreeMap::from([("smithing".to_owned(), 9.0)]),
            ..Profile::default()
        };
        let small = tree.award_main(&before, "mining", 1.0);
        assert!(!tree.levels_changed(&before.xp, &small.xp));
        let crossed = tree.award_cross(&small, "smithing", 5.0);
        assert!(tree.levels_changed(&before.xp, &crossed.xp));
        let big = tree.award_main(&before, "mining", 10.0);
        assert!(tree.levels_changed(&before.xp, &big.xp));
    }

    #[test]
    fn experience_added_in_place_matches_the_awarded_copies() {
        let tree = Tree::parse(TREE.as_bytes()).unwrap();
        let start = Profile {
            xp: BTreeMap::from([("smithing".to_owned(), 9.0)]),
            general_xp: 2.0,
            ..Profile::default()
        };
        let mut in_place = start.clone();
        tree.add_main_xp(&mut in_place, "mining", 3.0);
        tree.add_cross_xp(&mut in_place, "smithing", 5.0);
        let copied = tree.award_cross(&tree.award_main(&start, "mining", 3.0), "smithing", 5.0);
        assert_eq!(in_place, copied);
        assert_eq!(in_place.xp["mining"], 3.0);
        assert_eq!(in_place.xp["smithing"], 9.0 + 5.0 * 0.8);
        assert_eq!(in_place.general_xp, 2.0 + 3.0 * 0.5);
    }

    const RANKED_TREE: &str = r##"{
        "start": 0, "general_share": 0.5, "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 3},
        "branches": {"mining": {"name": "M", "colour": "#ff3333", "icon": "branch:mining"}},
        "base_attributes": {"minecraft:luck": 0.0},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "start", "name": "s", "kind": "start", "links": [1]},
            {"id": 1, "x": 1, "y": 0, "icon": "a", "name": "r", "kind": "upgrade", "branch": "mining",
             "links": [2], "effects": {"k": 0.1}, "ranks": 2,
             "bonuses": [{"attribute": "minecraft:luck", "amount": 0.5}],
             "rank_descriptions": ["10%", "20%"]},
            {"id": 2, "x": 2, "y": 0, "icon": "a", "name": "n", "kind": "small", "branch": "mining",
             "effects": {"m": 1.0}}
        ]
    }"##;

    /// A ranked tree's player with `levels` mining levels (the curve's 10 xp per level).
    fn ranked(levels: f64) -> (Tree, Profile) {
        let tree = Tree::parse(RANKED_TREE.as_bytes()).unwrap();
        let mut profile = tree.new_profile();
        profile.xp.insert(
            "mining".to_owned(),
            [0.0, 10.0, 30.0, 60.0][levels as usize],
        );
        (tree, profile)
    }

    #[test]
    fn each_click_adds_one_rank_up_to_the_node_ranks() {
        let (tree, player) = ranked(3.0);
        let once = tree.allocate(&player, 1).unwrap();
        assert_eq!(once.rank(1), 1);
        let twice = tree.allocate(&once, 1).unwrap();
        assert_eq!(twice.rank(1), 2);
        assert_eq!(tree.allocate(&twice, 1), Err(Refusal::AlreadyAllocated));
    }

    #[test]
    fn a_neighbour_opens_only_when_the_ranked_node_is_full() {
        let (tree, player) = ranked(3.0);
        let once = tree.allocate(&player, 1).unwrap();
        assert!(!tree.is_full(&once, 1));
        assert_eq!(tree.allocate(&once, 2), Err(Refusal::NotLinked));
        let twice = tree.allocate(&once, 1).unwrap();
        assert!(tree.is_full(&twice, 1));
        assert_eq!(tree.allocate(&twice, 2).unwrap().rank(2), 1);
    }

    #[test]
    fn effects_and_bonuses_scale_with_the_rank() {
        let (tree, player) = ranked(3.0);
        let once = tree.allocate(&player, 1).unwrap();
        let twice = tree.allocate(&once, 1).unwrap();
        assert_eq!(tree.effects(&once).get("k"), Some(&0.1));
        assert_eq!(tree.effects(&twice).get("k"), Some(&0.2));
        assert_eq!(tree.attributes(&twice.ranks)["minecraft:luck"], 1.0);
    }

    #[test]
    fn every_rank_costs_one_point() {
        let (tree, player) = ranked(2.0);
        let once = tree.allocate(&player, 1).unwrap();
        assert_eq!(tree.branch_points(&once, "mining"), 1);
        let twice = tree.allocate(&once, 1).unwrap();
        assert_eq!(tree.branch_points(&twice, "mining"), 0);
        assert_eq!(tree.allocate(&twice, 2), Err(Refusal::NoPointsLeft));
    }

    #[test]
    fn respec_clears_ranks_and_sanitize_keeps_a_partial_rank_without_opening_on() {
        let (tree, player) = ranked(3.0);
        let once = tree.allocate(&player, 1).unwrap();
        assert_eq!(tree.respec(&once).ranks, learned(&[0]));
        let mut saved = once.clone();
        saved.ranks.insert(2, 1);
        assert_eq!(tree.sanitize(&saved).ranks, Ranks::from([(0, 1), (1, 1)]));
    }

    #[test]
    fn an_old_profile_loads_with_experience_kept_and_nodes_dropped() {
        let (tree, _) = ranked(0.0);
        let old = br#"{"allocated":[0,1,2],"xp":{"mining":55.0},"general_xp":12.5,
            "seen":["biome:minecraft:desert"],"disabled":[1]}"#;
        let (profile, dropped) = tree.load_saved(old).unwrap();
        assert!(dropped);
        assert_eq!(profile.layout, PROFILE_LAYOUT);
        assert_eq!(profile.ranks, learned(&[0]));
        assert!(profile.disabled.is_empty());
        assert_eq!(profile.xp["mining"], 55.0);
        assert_eq!(profile.general_xp, 12.5);
        assert!(profile.seen.contains("biome:minecraft:desert"));
    }

    #[test]
    fn the_client_copy_is_the_profile_without_one_time_rewards() {
        let (tree, player) = ranked(3.0);
        let mut profile = tree.allocate(&player, 1).unwrap();
        profile.seen = BTreeSet::from(["biome:minecraft:plains".to_owned()]);
        profile.disabled = Allocated::from([1]);
        let bytes = serde_json::to_vec(&profile.for_client()).unwrap();
        let shown: Profile = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            shown,
            Profile {
                seen: BTreeSet::new(),
                ..profile.clone()
            }
        );
        assert!(bytes.len() < serde_json::to_vec(&profile).unwrap().len());
    }

    #[test]
    fn a_current_profile_round_trips_with_its_ranks() {
        let (tree, player) = ranked(3.0);
        let once = tree.allocate(&player, 1).unwrap();
        let bytes = serde_json::to_vec(&once).unwrap();
        let (loaded, dropped) = tree.load_saved(&bytes).unwrap();
        assert!(!dropped);
        assert_eq!(loaded, once);
        assert!(tree.load_saved(b"not json").is_err());
    }

    const UPGRADE_TREE: &str = r#"{
        "start": 0, "general_share": 0.5, "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 3},
        "branches": {"mining": {"name": "M"}},
        "level_bonus": {"mining": "speed"},
        "base_attributes": {},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "a", "name": "s", "kind": "start", "links": [1]},
            {"id": 1, "x": 1, "y": 0, "icon": "a", "name": "b", "kind": "big", "branch": "mining",
             "links": [2], "effects": {"k": 1.0}},
            {"id": 2, "x": 2, "y": 0, "icon": "a", "name": "u", "kind": "upgrade",
             "branch": "mining", "effects": {"k": 2.0}}
        ]
    }"#;

    const TREE: &str = r#"{
        "start": 0,
        "general_share": 0.5,
        "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 3},
        "branches": {
            "mining": {"name": "Шахтёр", "tool_suffix": "_pickaxe",
                       "xp": {"minecraft:blocks/stone": 0.5, "minecraft:blocks/iron_ore": 8.0}},
            "smithing": {"name": "Кузнец"}
        },
        "base_attributes": {"minecraft:max_health": 20.0},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "a", "name": "s", "kind": "start", "links": [1, 2]},
            {"id": 1, "x": 1, "y": 0, "icon": "a", "name": "a", "kind": "small", "branch": "mining",
             "links": [3], "effects": {"mining_speed": 0.1}},
            {"id": 2, "x": 2, "y": 0, "icon": "a", "name": "b", "kind": "small", "branch": "smithing",
             "bonuses": [{"attribute": "minecraft:max_health", "amount": 3.0}]},
            {"id": 3, "x": 3, "y": 0, "icon": "a", "name": "c", "kind": "big", "branch": "mining",
             "effects": {"mining_speed": 0.2, "vein_len": 8.0}, "todo": true}
        ]
    }"#;

    fn tree() -> Tree {
        Tree::parse(TREE.as_bytes()).expect("valid tree")
    }

    /// A profile with `levels` in mining, and `general` general levels.
    fn profile(tree: &Tree, levels: f64, general: f64) -> Profile {
        let mut profile = tree.new_profile();
        profile.xp.insert("mining".to_owned(), levels);
        profile.general_xp = general;
        profile
    }

    #[test]
    fn levels_follow_the_curve_and_stop_at_the_cap() {
        let tree = tree();
        assert_eq!(tree.level_progress(0.0).0, 0);
        assert_eq!(tree.level_progress(9.9).0, 0);
        assert_eq!(tree.level_progress(10.0).0, 1);
        assert_eq!(tree.level_progress(30.0).0, 2);
        assert_eq!(tree.level_progress(1_000.0), (3, 1.0));
    }

    #[test]
    fn a_new_player_cannot_buy_anything() {
        let tree = tree();
        assert_eq!(
            tree.allocate(&tree.new_profile(), 1),
            Err(Refusal::NoPointsLeft)
        );
    }

    #[test]
    fn a_branch_point_buys_a_node_of_that_branch_without_changing_the_old_profile() {
        let tree = tree();
        let before = profile(&tree, 10.0, 0.0);
        let after = tree.allocate(&before, 1).expect("paid by mining");
        assert!(after.is_learned(1));
        assert!(!before.is_learned(1));
        assert_eq!(tree.branch_points(&after, "mining"), 0);
    }

    #[test]
    fn a_branch_point_does_not_pay_for_another_branch() {
        let tree = tree();
        let player = profile(&tree, 10.0, 0.0);
        assert_eq!(tree.allocate(&player, 2), Err(Refusal::NoPointsLeft));
    }

    #[test]
    fn general_points_pay_for_any_branch_after_its_own_points_run_out() {
        let tree = tree();
        let player = profile(&tree, 10.0, 10.0);
        let first = tree.allocate(&player, 1).expect("own point");
        assert_eq!(tree.general_points(&first), 1);
        let second = tree.allocate(&first, 3).expect("general point");
        assert_eq!(tree.general_points(&second), 0);
    }

    #[test]
    fn unlinked_allocated_and_unknown_nodes_are_refused() {
        let tree = tree();
        let player = profile(&tree, 1_000.0, 1_000.0);
        assert_eq!(tree.allocate(&player, 3), Err(Refusal::NotLinked));
        assert_eq!(tree.allocate(&player, 0), Err(Refusal::AlreadyAllocated));
        assert_eq!(tree.allocate(&player, 99), Err(Refusal::UnknownNode));
    }

    #[test]
    fn main_work_feeds_the_general_scale_and_related_work_does_not() {
        let tree = tree();
        let main = tree.award_main(&tree.new_profile(), "mining", 8.0);
        assert_eq!(main.xp["mining"], 8.0);
        assert_eq!(main.general_xp, 4.0);
        let cross = tree.award_cross(&main, "smithing", 5.0);
        assert_eq!(cross.xp["smithing"], 4.0);
        assert_eq!(cross.general_xp, 4.0);
    }

    #[test]
    fn experience_needs_the_right_tool_and_a_listed_block() {
        let tree = tree();
        assert_eq!(
            tree.xp_for_break("minecraft:blocks/iron_ore", "minecraft:iron_pickaxe"),
            Some(("mining", 8.0))
        );
        assert_eq!(
            tree.xp_for_break("minecraft:blocks/iron_ore", "minecraft:iron_shovel"),
            None
        );
        assert_eq!(
            tree.xp_for_break("minecraft:blocks/dirt", "minecraft:iron_pickaxe"),
            None
        );
    }

    #[test]
    fn actions_lists_and_numbers_are_read_from_the_tree() {
        let json = r#"{"start":0,"general_share":0.1,"cross_share":0.8,
            "curve":{"coefficient":1,"exponent":1,"max_level":1},
            "branches":{"fishing":{"name":"Р","xp":{"act:fish":18}}},
            "lists":{"fish":["minecraft:cod"],"gear":["_sword"]},
            "numbers":{"mob_health":{"minecraft:zombie":20}},
            "nodes":[{"id":0,"x":0,"y":0,"icon":"a","name":"s","kind":"start"}]}"#;
        let tree = Tree::parse(json.as_bytes()).expect("valid tree");
        assert_eq!(tree.xp_for_action("act:fish"), Some(("fishing", 18.0)));
        assert_eq!(tree.xp_for_action("act:none"), None);
        assert!(tree.in_list("fish", "minecraft:cod"));
        assert!(!tree.in_list("fish", "minecraft:boot"));
        assert!(tree.has_suffix_in("gear", "minecraft:iron_sword"));
        assert_eq!(tree.number("mob_health", "minecraft:zombie"), Some(20.0));
    }

    #[test]
    fn only_listed_blocks_count_as_earning_experience() {
        let tree = tree();
        assert!(tree.earns_xp("minecraft:blocks/stone"));
        assert!(!tree.earns_xp("minecraft:blocks/dirt"));
    }

    #[test]
    fn effects_add_up_and_unimplemented_nodes_do_not_count() {
        let tree = tree();
        let profile = Profile {
            ranks: learned(&[0, 1, 3]),
            ..Profile::default()
        };
        let effects = tree.effects(&profile);
        assert_eq!(effects["mining_speed"], 0.1);
        assert!(!effects.contains_key("vein_len"));
    }

    #[test]
    fn sanitize_drops_unknown_and_disconnected_nodes_and_keeps_experience() {
        let tree = tree();
        let mut saved = profile(&tree, 1_000.0, 1_000.0);
        saved.ranks = learned(&[0, 3, 99]);
        let kept = tree.sanitize(&saved);
        assert_eq!(kept.ranks, learned(&[0]));
        assert_eq!(kept.xp["mining"], 1_000.0);
    }

    #[test]
    fn sanitize_drops_what_can_no_longer_be_paid_for() {
        let tree = tree();
        let mut saved = profile(&tree, 10.0, 0.0);
        saved.ranks = learned(&[0, 1, 3]);
        assert_eq!(tree.sanitize(&saved).ranks, learned(&[0, 1]));
    }

    #[test]
    fn broken_trees_are_rejected_with_a_reason() {
        let base = |body: &str| {
            format!(
                r#"{{"start":0,"general_share":0.1,"cross_share":0.8,
                "curve":{{"coefficient":1,"exponent":1,"max_level":1}},
                "branches":{{}},"base_attributes":{{}},"nodes":[{body}]}}"#
            )
        };
        let node = |extra: &str| {
            format!(r#"{{"id":0,"x":0,"y":0,"icon":"a","name":"a","kind":"start"{extra}}}"#)
        };
        let cases = [
            (base(""), "start node"),
            (base(&node(r#","links":[7]"#)), "unknown node"),
            (base(&format!("{},{}", node(""), node(""))), "defined twice"),
            (base(&node(r#","branch":"nope""#)), "unknown branch"),
            (
                base(&node(
                    r#","bonuses":[{"attribute":"minecraft:luck","amount":1}]"#,
                )),
                "base_attributes",
            ),
        ];
        for (json, reason) in cases {
            let error = Tree::parse(json.as_bytes()).expect_err(&json);
            assert!(error.contains(reason), "{error}");
        }
    }

    #[test]
    fn a_large_payload_is_split_under_the_cap_and_reassembled() {
        let payload: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
        let parts = split_parts(&payload);
        assert_eq!(parts.len(), 3);
        assert!(
            parts
                .iter()
                .all(|part| part.len() <= MAX_PART + PART_HEADER)
        );
        let mut assembler = Assembler::default();
        let results: Vec<_> = parts.iter().map(|part| assembler.push(part)).collect();
        assert_eq!(results[0], None);
        assert_eq!(results[2].as_deref(), Some(&payload[..]));
    }

    #[test]
    fn a_malformed_part_or_number_is_ignored() {
        let mut assembler = Assembler::default();
        assert_eq!(assembler.push(&[1, 0]), None);
        assert_eq!(assembler.push(&[5, 0, 2, 0, 9]), None);
        assert_eq!(decode_u16(&[1]), None);
        assert_eq!(decode_u16(&[1, 0]), Some(1));
    }
}
