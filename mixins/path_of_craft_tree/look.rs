// What the tree window works out once per tree, free of game types: distances from the start
// (the way learned energy flows), branch banners and name spots, the colour zones behind the
// branches, and how each node looks for a player. The approved preview (design/preview/tree.js
// and render.js) is the reference for every number here.
use crate::game::pct_rules::{Allocated, Kind, Points, Profile, Tree};
use std::collections::BTreeMap;

/// Particles along learned links: world units per second, and the gap between them on a chain.
pub const FLOW_SPEED: f32 = 80.0;
pub const FLOW_PERIOD: f32 = 110.0;
/// Zone tint per level, and how strongly a zone pulls the stone towards its branch colour.
const ZONE_LEVELS: f32 = 4.0;
const ZONE_ALPHA_STEP: f32 = 0.13;
const ZONE_INK: f32 = 0.24;
/// Far-out branch names: the free gap kept between two names, the step outward while they still
/// collide, and how far a name may travel before it is left where it is.
const LABEL_GAP: f32 = 2.0;
const LABEL_STEP: f32 = 2.0;
const LABEL_STEPS: u16 = 200;

/// How a node is drawn: its frame colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Locked,
    Available,
    Partial,
    Learned,
    Off,
}

impl State {
    pub const ALL: [State; 5] = [
        State::Locked,
        State::Available,
        State::Partial,
        State::Learned,
        State::Off,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            State::Locked => "locked",
            State::Available => "available",
            State::Partial => "partial",
            State::Learned => "learned",
            State::Off => "off",
        }
    }
}

/// The frame side in sprite pixels: full frames (drawn 1 to 3 times bigger) and compact ones.
pub fn frame_side(kind: Kind, compact: bool) -> f32 {
    match (kind, compact) {
        (Kind::Small | Kind::Upgrade, false) => 24.0,
        (Kind::Big, false) => 32.0,
        (Kind::Start, false) => 44.0,
        (Kind::Small | Kind::Upgrade, true) => 12.0,
        (Kind::Big, true) => 16.0,
        (Kind::Start, true) => 20.0,
    }
}

pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Small => "small",
        Kind::Upgrade => "upgrade",
        Kind::Big => "big",
        Kind::Start => "start",
    }
}

/// Frame scale for a zoom: 1 to 3 full frames, 0.5 compact frames, 0 plain dots.
pub fn level(spacing: f32, zoom: f32) -> f32 {
    let room = spacing * zoom * 0.8;
    for scale in [3.0, 2.0, 1.0] {
        if 24.0 * scale <= room {
            return scale;
        }
    }
    if room >= 11.0 { 0.5 } else { 0.0 }
}

/// Half the side of the area that takes a node's clicks, in GUI pixels.
pub fn hit_half(kind: Kind, level: f32) -> f32 {
    if level >= 1.0 {
        frame_side(kind, false) / 2.0 * level
    } else if level > 0.0 {
        frame_side(kind, true) / 2.0
    } else if matches!(kind, Kind::Big | Kind::Start) {
        4.0
    } else {
        3.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchLook {
    pub id: String,
    pub name: String,
    /// 0xRRGGBB.
    pub colour: u32,
    /// Where the banner hangs, near the centre.
    pub anchor: [f32; 2],
    /// Where the name goes when zoomed far out.
    pub label: [f32; 2],
}

/// A coarse world grid behind the tree: each cell's branch and tint level (0 is untinted).
#[derive(Debug, Clone, PartialEq)]
pub struct Zones {
    pub x0: f32,
    pub y0: f32,
    pub cell: f32,
    pub cols: usize,
    pub rows: usize,
    pub branch: Vec<u8>,
    pub level: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// Shortest distance from the start along links.
    pub dist: BTreeMap<u16, f32>,
    /// Each link once, lower id first.
    pub edges: Vec<(u16, u16)>,
    /// The median link length away from the start.
    pub spacing: f32,
    pub branches: Vec<BranchLook>,
    pub zones: Zones,
    /// Small nodes first and the start last, so bigger frames are drawn over smaller ones.
    pub draw_order: Vec<u16>,
}

fn kind_order(kind: Kind) -> u8 {
    match kind {
        Kind::Small => 0,
        Kind::Upgrade => 1,
        Kind::Big => 2,
        Kind::Start => 3,
    }
}

fn length(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn median(mut values: Vec<f32>, empty: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(empty)
}

fn mean(points: &[[f32; 2]]) -> [f32; 2] {
    let count = points.len().max(1) as f32;
    let sum = points
        .iter()
        .fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
    [sum[0] / count, sum[1] / count]
}

/// "#rrggbb" as 0xRRGGBB; grey for anything else.
pub fn parse_colour(text: Option<&str>) -> u32 {
    text.and_then(|t| t.strip_prefix('#'))
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .unwrap_or(0x808080)
}

/// `a` moved towards `b` by `t`, both 0xRRGGBB.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    [16, 8, 0].iter().fold(0, |out, shift| {
        let (x, y) = (((a >> shift) & 255) as f32, ((b >> shift) & 255) as f32);
        out | (((x * (1.0 - t) + y * t).round() as u32) << shift)
    })
}

/// Shortest distances from the start along links: Dijkstra with a binary heap. Distances are
/// never negative, so the bits of an `f32` order like the number itself.
fn distances(tree: &Tree) -> BTreeMap<u16, f32> {
    use std::cmp::Reverse;
    let mut dist: BTreeMap<u16, f32> = tree.nodes.keys().map(|id| (*id, f32::INFINITY)).collect();
    dist.insert(tree.start, 0.0);
    let mut open = std::collections::BinaryHeap::from([Reverse((0.0f32.to_bits(), tree.start))]);
    while let Some(Reverse((bits, id))) = open.pop() {
        let here = f32::from_bits(bits);
        if dist.get(&id).is_some_and(|best| here > *best) {
            continue;
        }
        let Some(node) = tree.nodes.get(&id) else {
            continue;
        };
        for link in &node.links {
            let Some(far) = tree.nodes.get(link) else {
                continue;
            };
            let through = here + length([node.x, node.y], [far.x, far.y]);
            if let Some(d) = dist.get_mut(link)
                && through < *d
            {
                *d = through;
                open.push(Reverse((through.to_bits(), *link)));
            }
        }
    }
    dist
}

fn branch_looks(tree: &Tree, spacing: f32) -> Vec<BranchLook> {
    let start = &tree.nodes[&tree.start];
    let origin = [start.x, start.y];
    tree.branches
        .iter()
        .map(|(id, branch)| {
            let own: Vec<_> = tree
                .nodes
                .values()
                .filter(|n| n.branch.as_deref() == Some(id))
                .collect();
            let points: Vec<[f32; 2]> = own.iter().map(|n| [n.x, n.y]).collect();
            let roots: Vec<[f32; 2]> = own
                .iter()
                .filter(|n| n.links.contains(&tree.start))
                .map(|n| [n.x, n.y])
                .collect();
            let centroid = mean(&points);
            let toward = if roots.is_empty() {
                centroid
            } else {
                mean(&roots)
            };
            let reach = length(toward, origin).max(1.0);
            let along = (reach * 0.72).min((reach - 1.5 * spacing).max(reach * 0.4));
            let mut radii: Vec<f32> = points.iter().map(|p| length(*p, origin)).collect();
            radii.sort_by(f32::total_cmp);
            let radius = radii.get(radii.len() * 3 / 4).copied().unwrap_or(0.0);
            let spread = length(centroid, origin).max(1.0);
            BranchLook {
                id: id.clone(),
                name: branch.name.clone(),
                colour: parse_colour(branch.colour.as_deref()),
                anchor: [
                    origin[0] + (toward[0] - origin[0]) / reach * along,
                    origin[1] + (toward[1] - origin[1]) / reach * along,
                ],
                label: [
                    origin[0] + (centroid[0] - origin[0]) / spread * radius,
                    origin[1] + (centroid[1] - origin[1]) / spread * radius,
                ],
            }
        })
        .collect()
}

/// Screen boxes `[x, y, width, height]` for names wanted centred at the given spots: names
/// nearest `centre` keep their spot, and each later one moves straight away from `centre` until
/// it runs into none placed before it.
pub fn spread_labels(centre: [f32; 2], wanted: &[([f32; 2], f32)], height: f32) -> Vec<[f32; 4]> {
    let mut order: Vec<usize> = (0..wanted.len()).collect();
    order.sort_by(|a, b| length(wanted[*a].0, centre).total_cmp(&length(wanted[*b].0, centre)));
    let mut boxes = vec![[0.0; 4]; wanted.len()];
    let mut placed: Vec<[f32; 4]> = Vec::with_capacity(wanted.len());
    for index in order {
        let (at, width) = wanted[index];
        let reach = length(at, centre);
        let way = if reach > 0.0 {
            [(at[0] - centre[0]) / reach, (at[1] - centre[1]) / reach]
        } else {
            [0.0, -1.0]
        };
        let at_step = |step: u16| {
            let out = f32::from(step) * LABEL_STEP;
            let mid = [at[0] + way[0] * out, at[1] + way[1] * out];
            [mid[0] - width / 2.0, mid[1] - height / 2.0, width, height]
        };
        let spot = (0..=LABEL_STEPS)
            .map(at_step)
            .find(|b| !placed.iter().any(|p| collide(b, p, LABEL_GAP)))
            .unwrap_or_else(|| at_step(0));
        placed.push(spot);
        boxes[index] = spot;
    }
    boxes
}

fn collide(a: &[f32; 4], b: &[f32; 4], gap: f32) -> bool {
    a[0] < b[0] + b[2] + gap
        && b[0] < a[0] + a[2] + gap
        && a[1] < b[1] + b[3] + gap
        && b[1] < a[1] + a[3] + gap
}

/// Reach of the branches' first nodes: where the zones reach full strength.
fn first_ring(tree: &Tree) -> f32 {
    let start = &tree.nodes[&tree.start];
    let reaches = tree
        .branches
        .keys()
        .map(|id| {
            let roots: Vec<[f32; 2]> = start
                .links
                .iter()
                .filter_map(|link| tree.nodes.get(link))
                .filter(|n| n.branch.as_deref() == Some(id))
                .map(|n| [n.x, n.y])
                .collect();
            length(mean(&roots), [start.x, start.y])
        })
        .collect();
    median(reaches, 100.0)
}

/// A node seen by the zone map: its position and the index of its branch's colour.
type ZoneNode = (f32, f32, u8);

/// Zone nodes sorted into square buckets of side `size`, kept in a flat grid: the zone map asks
/// for nine buckets around each of its tens of thousands of cells.
struct Buckets {
    left: i32,
    top: i32,
    cols: i32,
    rows: i32,
    nodes: Vec<Vec<ZoneNode>>,
}

impl Buckets {
    fn new(placed: &[ZoneNode], size: f32) -> Buckets {
        let at = |v: f32| (v / size).floor() as i32;
        let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for (x, y, _) in placed {
            (left, right) = (left.min(at(*x)), right.max(at(*x)));
            (top, bottom) = (top.min(at(*y)), bottom.max(at(*y)));
        }
        let (cols, rows) = if placed.is_empty() {
            (0, 0)
        } else {
            (right - left + 1, bottom - top + 1)
        };
        let mut buckets = Buckets {
            left,
            top,
            cols,
            rows,
            nodes: vec![Vec::new(); (cols * rows).max(0) as usize],
        };
        for node in placed {
            let index = buckets.index(at(node.0), at(node.1));
            if let Some(bucket) = index.and_then(|i| buckets.nodes.get_mut(i)) {
                bucket.push(*node);
            }
        }
        buckets
    }

    fn index(&self, bx: i32, by: i32) -> Option<usize> {
        let (c, r) = (bx - self.left, by - self.top);
        (c >= 0 && r >= 0 && c < self.cols && r < self.rows).then(|| (r * self.cols + c) as usize)
    }

    /// The nodes in bucket (`bx`, `by`), counted in bucket sides from the origin.
    fn get(&self, bx: i32, by: i32) -> &[ZoneNode] {
        self.index(bx, by)
            .and_then(|i| self.nodes.get(i))
            .map_or(&[], Vec::as_slice)
    }
}

fn zones(tree: &Tree, spacing: f32, branches: &[BranchLook]) -> Zones {
    let index: BTreeMap<&str, u8> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.as_str(), i as u8))
        .collect();
    let placed: Vec<ZoneNode> = tree
        .nodes
        .values()
        .filter_map(|n| {
            let b = index.get(n.branch.as_deref()?)?;
            Some((n.x, n.y, *b))
        })
        .collect();
    let cell = (spacing * 0.6).round().max(12.0);
    let (fade_near, fade_far) = (spacing * 1.2, spacing * 4.0);
    let bounds = placed
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, (x, y, _)| {
            [b[0].min(*x), b[1].min(*y), b[2].max(*x), b[3].max(*y)]
        });
    let (x0, y0) = (bounds[0] - fade_far, bounds[1] - fade_far);
    let cols = ((bounds[2] + fade_far - x0) / cell).ceil().max(0.0) as usize;
    let rows = ((bounds[3] + fade_far - y0) / cell).ceil().max(0.0) as usize;
    let buckets = Buckets::new(&placed, fade_far);
    let start = &tree.nodes[&tree.start];
    let ring = first_ring(tree);
    let inner = ring * 0.25;
    let full = (ring * 0.85).max(inner + spacing);
    let mut zones = Zones {
        x0,
        y0,
        cell,
        cols,
        rows,
        branch: vec![0; cols * rows],
        level: vec![0; cols * rows],
    };
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (x0 + (c as f32 + 0.5) * cell, y0 + (r as f32 + 0.5) * cell);
            let (bx, by) = ((x / fade_far).floor() as i32, (y / fade_far).floor() as i32);
            // The nearest node by squared distance; only the winner's distance is worked out.
            let mut best: Option<(f32, f32, f32, u8)> = None;
            for j in -1..=1 {
                for i in -1..=1 {
                    for (nx, ny, b) in buckets.get(bx + i, by + j) {
                        let squared = (nx - x) * (nx - x) + (ny - y) * (ny - y);
                        if squared < best.map_or(fade_far * fade_far, |found| found.0) {
                            best = Some((squared, *nx, *ny, *b));
                        }
                    }
                }
            }
            let Some((_, nx, ny, b)) = best else { continue };
            let d = length([nx, ny], [x, y]);
            if d >= fade_far {
                continue;
            }
            let outer = 1.0 - ((d - fade_near) / (fade_far - fade_near)).clamp(0.0, 1.0);
            let from_start = length([x, y], [start.x, start.y]);
            let centre = ((from_start - inner) / (full - inner)).clamp(0.0, 1.0);
            let k = (outer * centre * ZONE_LEVELS).round() as u8;
            if k > 0 {
                zones.branch[r * cols + c] = b;
                zones.level[r * cols + c] = k;
            }
        }
    }
    zones
}

impl Look {
    pub fn new(tree: &Tree) -> Look {
        let mut edges = Vec::new();
        for node in tree.nodes.values() {
            for link in node.links.iter().filter(|link| **link > node.id) {
                edges.push((node.id, *link));
            }
        }
        let away: Vec<f32> = edges
            .iter()
            .filter(|(a, b)| *a != tree.start && *b != tree.start)
            .map(|(a, b)| {
                let (p, q) = (&tree.nodes[a], &tree.nodes[b]);
                length([p.x, p.y], [q.x, q.y])
            })
            .collect();
        let spacing = median(away, 40.0);
        let branches = branch_looks(tree, spacing);
        let zones = zones(tree, spacing, &branches);
        let mut draw_order: Vec<u16> = tree.nodes.keys().copied().collect();
        draw_order.sort_by_key(|id| kind_order(tree.nodes[id].kind));
        Look {
            dist: distances(tree),
            edges,
            spacing,
            branches,
            zones,
            draw_order,
        }
    }

    /// The zones as `cols` x `rows` RGBA pixels to lay over dark stone: each branch colour at
    /// a strength that matches the preview's tinted stone.
    pub fn zone_pixels(&self) -> Vec<u8> {
        let z = &self.zones;
        let mut out = vec![0u8; z.cols * z.rows * 4];
        for (i, (branch, level)) in z.branch.iter().zip(&z.level).enumerate() {
            if *level == 0 {
                continue;
            }
            let colour = self
                .branches
                .get(usize::from(*branch))
                .map_or(0, |b| b.colour);
            let ink = |shift: u32| (((colour >> shift) & 255) as f32 * ZONE_INK).round() as u8;
            out[i * 4] = ink(16);
            out[i * 4 + 1] = ink(8);
            out[i * 4 + 2] = ink(0);
            out[i * 4 + 3] = (f32::from(*level) * ZONE_ALPHA_STEP * 255.0).round() as u8;
        }
        out
    }

    pub fn branch(&self, id: &str) -> Option<&BranchLook> {
        self.branches.iter().find(|b| b.id == id)
    }
}

/// How far along a learned link (0 to 1) its particle is after `seconds`; none while the
/// particle is between runs. `start_distance` is the near end's distance from the start.
pub fn flow_at(seconds: f32, start_distance: f32, link_length: f32) -> Option<f32> {
    let period = FLOW_PERIOD.max(link_length);
    let travelled = (seconds * FLOW_SPEED - start_distance).rem_euclid(period);
    (travelled <= link_length && link_length > 0.0).then(|| travelled / link_length)
}

/// Points left for the player, worked out once so every node's look is cheap.
pub struct Standing<'a> {
    tree: &'a Tree,
    profile: &'a Profile,
    points: Points<'a>,
    off: Allocated,
}

impl<'a> Standing<'a> {
    pub fn new(tree: &'a Tree, profile: &'a Profile) -> Standing<'a> {
        Standing {
            tree,
            profile,
            points: tree.points(profile),
            off: tree.switched_off(profile),
        }
    }

    pub fn is_full(&self, id: u16) -> bool {
        self.tree.is_full(self.profile, id)
    }

    pub fn rank(&self, id: u16) -> u8 {
        self.profile.rank(id)
    }

    pub fn is_off(&self, id: u16) -> bool {
        self.off.contains(&id)
    }

    /// Not full yet and next to a full node, points or not.
    pub fn is_open(&self, id: u16) -> bool {
        !self.is_full(id) && self.tree.is_reachable(self.profile, id)
    }

    fn can_pay(&self, id: u16) -> bool {
        self.tree
            .nodes
            .get(&id)
            .is_some_and(|node| self.points.can_pay(node))
    }

    pub fn state(&self, id: u16) -> State {
        let learned = self.profile.is_learned(id);
        if learned && self.is_off(id) {
            State::Off
        } else if self.is_full(id) {
            State::Learned
        } else if learned {
            State::Partial
        } else if self.is_open(id) && self.can_pay(id) {
            State::Available
        } else {
            State::Locked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn crowded_names_are_pushed_apart_and_outward() {
        let centre = [0.0, 0.0];
        // Three wide names near the top, as the far-out view puts them.
        let wanted = [
            ([-30.0, -100.0], 80.0),
            ([0.0, -104.0], 60.0),
            ([25.0, -98.0], 50.0),
        ];
        let boxes = spread_labels(centre, &wanted, 9.0);
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                assert!(!overlaps(*a, *b), "{a:?} runs into {b:?}");
            }
        }
        for (b, (at, width)) in boxes.iter().zip(wanted) {
            assert_eq!(b[2], width);
            let mid = [b[0] + b[2] / 2.0, b[1] + b[3] / 2.0];
            assert!(length(mid, centre) >= length(at, centre) - 0.01);
        }
    }

    #[test]
    fn names_with_room_stay_where_they_are() {
        let boxes = spread_labels(
            [0.0, 0.0],
            &[([100.0, 0.0], 40.0), ([-100.0, 0.0], 40.0)],
            9.0,
        );
        assert_eq!(boxes[0], [80.0, -4.5, 40.0, 9.0]);
        assert_eq!(boxes[1], [-120.0, -4.5, 40.0, 9.0]);
    }

    const TREE: &str = r##"{
        "start": 0, "general_share": 0.5, "cross_share": 0.8,
        "curve": {"coefficient": 10.0, "exponent": 1.0, "max_level": 3},
        "branches": {"a": {"name": "A", "colour": "#ff0000"}, "b": {"name": "B", "colour": "#0000ff"}},
        "base_attributes": {},
        "nodes": [
            {"id": 0, "x": 0, "y": 0, "icon": "start", "name": "s", "kind": "start", "links": [1, 3]},
            {"id": 1, "x": 0, "y": -100, "icon": "i", "name": "a1", "kind": "small", "branch": "a",
             "links": [2], "ranks": 2},
            {"id": 2, "x": 0, "y": -140, "icon": "i", "name": "a2", "kind": "big", "branch": "a"},
            {"id": 3, "x": 0, "y": 100, "icon": "i", "name": "b1", "kind": "small", "branch": "b",
             "links": [4]},
            {"id": 4, "x": 0, "y": 140, "icon": "i", "name": "b2", "kind": "small", "branch": "b"}
        ]
    }"##;

    fn tree() -> Tree {
        Tree::parse(TREE.as_bytes()).unwrap()
    }

    #[test]
    fn levels_step_with_zoom() {
        assert_eq!(level(40.0, 3.0), 3.0);
        assert_eq!(level(40.0, 1.0), 1.0);
        assert_eq!(level(40.0, 0.5), 0.5);
        assert_eq!(level(40.0, 0.1), 0.0);
        assert_eq!(hit_half(Kind::Big, 2.0), 32.0);
        assert_eq!(hit_half(Kind::Small, 0.5), 6.0);
    }

    #[test]
    fn distances_follow_links_and_branches_get_their_colours() {
        let look = Look::new(&tree());
        assert_eq!(look.dist[&2], 140.0);
        assert_eq!(look.edges.len(), 4);
        assert_eq!(look.spacing, 40.0);
        let a = look.branch("a").unwrap();
        assert_eq!(a.colour, 0xff0000);
        assert!(a.anchor[1] < 0.0 && a.label[1] < 0.0);
    }

    #[test]
    fn zones_tint_cells_near_branches_with_their_colour() {
        let look = Look::new(&tree());
        let z = &look.zones;
        assert_eq!(z.branch.len(), z.cols * z.rows);
        let tinted: Vec<usize> = (0..z.level.len()).filter(|i| z.level[*i] > 0).collect();
        assert!(!tinted.is_empty());
        let pixels = look.zone_pixels();
        let red = tinted
            .iter()
            .find(|i| look.branches[usize::from(z.branch[**i])].id == "a")
            .unwrap();
        assert!(pixels[red * 4] > 0 && pixels[red * 4 + 2] == 0 && pixels[red * 4 + 3] > 0);
    }

    #[test]
    fn a_particle_runs_along_its_link_then_waits() {
        assert_eq!(flow_at(0.0, 0.0, 40.0), Some(0.0));
        assert_eq!(flow_at(0.25, 0.0, 40.0), Some(0.5));
        assert_eq!(flow_at(1.0, 0.0, 40.0), None);
        assert_eq!(flow_at(0.25, 20.0, 40.0), Some(0.0));
    }

    #[test]
    fn node_states_follow_ranks_points_and_switches() {
        let tree = tree();
        let mut profile = tree.new_profile();
        profile.xp.insert("a".to_owned(), 30.0);
        let standing = Standing::new(&tree, &profile);
        assert_eq!(standing.state(0), State::Learned);
        assert_eq!(standing.state(1), State::Available);
        assert_eq!(standing.state(3), State::Locked);
        let once = tree.allocate(&profile, 1).unwrap();
        let standing = Standing::new(&tree, &once);
        assert_eq!(standing.state(1), State::Partial);
        assert!(!standing.is_open(2));
        assert_eq!(mix(0xffffff, 0x000000, 0.5), 0x808080);
    }
}
