// The tree's final shape: the generator's nodes with runs of identical upgrades merged
// into one ranked node, laid out again by layout.rs, renumbered, and given pixel-icon ids.
use serde_json::{Value, json};
use std::collections::HashMap;

#[path = "geometry.rs"]
mod geometry;
#[path = "layout.rs"]
mod layout;

use layout::{BranchShape, RingShape};

pub const OUTPUT: &str = "design/out/tree-preview.json";

struct Cluster {
    spine_index: usize,
    big: Value,
    upgrades: Vec<Value>,
}

struct Branch {
    spine: Vec<Value>,
    clusters: Vec<Cluster>,
}

struct Ring {
    from: (usize, usize),
    to: (usize, usize),
    bridge: Option<Value>,
}

/// The tree with the preview's nodes in place of the generator's, and an icon on each branch.
pub fn convert(tree: &Value) -> Value {
    let nodes = tree["nodes"].as_array().expect("nodes");
    let (order, branches, rings) = read(nodes);
    let mut preview = tree.clone();
    preview["nodes"] = Value::Array(rebuild(&nodes[0], &branches, &rings));
    let entries = preview["branches"].as_object_mut().expect("branches");
    for (index, id) in order.iter().enumerate() {
        let branch = entries.get_mut(id).expect("every branch has an entry");
        branch["icon"] = json!(format!("branch:{id}"));
        let hue = 360.0 * index as f64 / order.len() as f64;
        branch["colour"] = json!(hex_colour(hue, BRANCH_LIGHTNESS));
    }
    preview
}

/// Lightness of the branch colours: full saturation, medium-high lightness.
const BRANCH_LIGHTNESS: f64 = 0.6;

/// "#rrggbb" for a fully saturated hue (degrees) at the given HSL lightness.
fn hex_colour(hue: f64, lightness: f64) -> String {
    let chroma = 1.0 - (2.0 * lightness - 1.0).abs();
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let light = lightness - chroma / 2.0;
    let byte = |channel: f64| ((channel + light) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

fn id_of(node: &Value) -> u64 {
    node["id"].as_u64().expect("node id")
}

fn link_ids(node: &Value) -> Vec<u64> {
    node["links"]
        .as_array()
        .map(|links| links.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

fn kind(node: &Value) -> &str {
    node["kind"].as_str().unwrap_or_default()
}

/// Splits the generator's nodes into branches (spines and clusters) and ring links.
fn read(nodes: &[Value]) -> (Vec<String>, Vec<Branch>, Vec<Ring>) {
    let mut order: Vec<String> = Vec::new();
    for node in nodes {
        if let Some(branch) = node["branch"].as_str()
            && !order.iter().any(|known| known == branch) {
                order.push(branch.to_owned());
            }
    }
    let child_of: HashMap<u64, &Value> = nodes
        .iter()
        .filter(|node| kind(node) == "upgrade")
        .map(|node| (link_ids(node)[0], node))
        .collect();
    let mut spine_of: HashMap<u64, (usize, usize)> = HashMap::new();
    let branches: Vec<Branch> = order
        .iter()
        .enumerate()
        .map(|(b, id)| {
            let in_branch = |node: &&Value| node["branch"].as_str() == Some(id.as_str());
            let spine: Vec<Value> = nodes
                .iter()
                .filter(in_branch)
                .filter(|node| kind(node) == "small")
                .cloned()
                .collect();
            for (k, node) in spine.iter().enumerate() {
                spine_of.insert(id_of(node), (b, k));
            }
            let clusters = nodes
                .iter()
                .filter(in_branch)
                .filter(|node| kind(node) == "big")
                .map(|big| {
                    let mut upgrades = Vec::new();
                    let mut current = id_of(big);
                    while let Some(next) = child_of.get(&current) {
                        upgrades.push((*next).clone());
                        current = id_of(next);
                    }
                    Cluster {
                        spine_index: spine_of[&link_ids(big)[0]].1,
                        big: big.clone(),
                        upgrades: merge_ranks(&upgrades),
                    }
                })
                .collect();
            Branch { spine, clusters }
        })
        .collect();
    let mut rings = Vec::new();
    for node in nodes {
        let links = link_ids(node);
        if let Some(from) = spine_of.get(&id_of(node)) {
            for to in links.iter().skip(1) {
                rings.push(Ring {
                    from: *from,
                    to: spine_of[to],
                    bridge: None,
                });
            }
        } else if kind(node) == "small" && node.get("branch").is_none() {
            rings.push(Ring {
                from: spine_of[&links[0]],
                to: spine_of[&links[1]],
                bridge: Some(node.clone()),
            });
        }
    }
    (order, branches, rings)
}

/// Whether the generator added the node after the numbering saved profiles know.
fn is_added(node: &Value) -> bool {
    node["added"] == json!(true)
}

/// Merges each run of upgrades that give exactly the same effects into one node with ranks.
/// Added boosters already carry their ranks and are never merged, so no older node changes.
fn merge_ranks(upgrades: &[Value]) -> Vec<Value> {
    let mut runs: Vec<Vec<&Value>> = Vec::new();
    for upgrade in upgrades {
        match runs.last_mut() {
            Some(run)
                if !is_added(run[0])
                    && !is_added(upgrade)
                    && run[0]["effects"] == upgrade["effects"] =>
            {
                run.push(upgrade)
            }
            _ => runs.push(vec![upgrade]),
        }
    }
    runs.into_iter()
        .map(|run| {
            if run.len() == 1 {
                return run[0].clone();
            }
            let names: Vec<&str> = run
                .iter()
                .map(|node| node["name"].as_str().unwrap_or_default())
                .collect();
            let descriptions: Vec<Value> =
                run.iter().map(|node| node["description"].clone()).collect();
            let mut merged = run[0].clone();
            merged["name"] = json!(base_name(&names));
            merged["ranks"] = json!(run.len());
            merged["rank_descriptions"] = Value::Array(descriptions);
            merged["todo"] = json!(run.iter().any(|node| node["todo"] == json!(true)));
            merged
        })
        .collect()
}

/// "Жадность: 15%" and "Жадность: 20%" give "Жадность"; "Нюх: радиус" and "Нюх: радиус II"
/// give "Нюх: радиус".
fn base_name(names: &[&str]) -> String {
    let unnumbered = |name: &str| name.trim_end_matches(['I', 'V', 'X']).trim_end().to_owned();
    let first = unnumbered(names[0]);
    if names.iter().all(|name| unnumbered(name) == first) {
        return first;
    }
    let head = |name: &str| name.split(": ").next().unwrap_or(name).to_owned();
    let common = head(names[0]);
    assert!(
        names.iter().all(|name| head(name) == common),
        "no shared name in {names:?}"
    );
    common
}

fn shapes(branches: &[Branch], rings: &[Ring]) -> (Vec<BranchShape>, Vec<RingShape>) {
    let branch_shapes = branches
        .iter()
        .map(|branch| BranchShape {
            spine: branch.spine.len(),
            clusters: branch
                .clusters
                .iter()
                .map(|cluster| (cluster.spine_index, cluster.upgrades.len()))
                .collect(),
        })
        .collect();
    let ring_shapes = rings
        .iter()
        .map(|ring| RingShape {
            from: ring.from,
            to: ring.to,
            bridge: ring.bridge.is_some(),
        })
        .collect();
    (branch_shapes, ring_shapes)
}

/// One coordinate, to a tenth of a unit.
fn coordinate(value: f64) -> Value {
    json!((value * 10.0).round() / 10.0)
}

/// Numbers the nodes in the order they are written, except added ones: those take the ids after
/// every older node, so the ids saved profiles refer to keep meaning the same nodes.
struct Writer {
    nodes: HashMap<u64, Value>,
    next: u64,
    next_added: u64,
}

impl Writer {
    fn push(&mut self, node: &Value, at: geometry::P, links: Vec<u64>, icon: String) -> u64 {
        let counter = if is_added(node) {
            &mut self.next_added
        } else {
            &mut self.next
        };
        let id = *counter;
        *counter += 1;
        let mut node = node.clone();
        if let Some(fields) = node.as_object_mut() {
            fields.remove("added");
        }
        node["id"] = json!(id);
        node["x"] = coordinate(at.x);
        node["y"] = coordinate(at.y);
        node["links"] = json!(links);
        node["icon"] = json!(icon);
        self.nodes.insert(id, node);
        id
    }

    /// The nodes in id order; the ids must run without a gap.
    fn finish(mut self) -> Vec<Value> {
        let all: Vec<Value> = (0..self.nodes.len() as u64)
            .map(|id| self.nodes.remove(&id).expect("ids without a gap"))
            .collect();
        assert!(self.nodes.is_empty(), "ids without a gap");
        all
    }
}

/// How many nodes are not added: the start, the branches' older nodes and the bridges.
fn older_count(branches: &[Branch], rings: &[Ring]) -> u64 {
    let in_branches: usize = branches
        .iter()
        .map(|branch| {
            let spine = branch.spine.iter().filter(|n| !is_added(n)).count();
            let clusters: usize = branch
                .clusters
                .iter()
                .map(|c| 1 + c.upgrades.iter().filter(|n| !is_added(n)).count())
                .sum();
            spine + clusters
        })
        .sum();
    let bridges = rings.iter().filter(|ring| ring.bridge.is_some()).count();
    (1 + in_branches + bridges) as u64
}

fn small_icon(node: &Value) -> String {
    let effects = node["effects"].as_object().expect("small effects");
    assert_eq!(effects.len(), 1, "a small node has one effect");
    format!("small:{}", effects.keys().next().expect("one key"))
}

fn rebuild(start: &Value, branches: &[Branch], rings: &[Ring]) -> Vec<Value> {
    let (branch_shapes, ring_shapes) = shapes(branches, rings);
    let layout = layout::lay_out(&branch_shapes, &ring_shapes);
    let mut start = start.clone();
    start["icon"] = json!("start");
    let mut writer = Writer {
        nodes: HashMap::from([(0, start)]),
        next: 1,
        next_added: older_count(branches, rings),
    };
    let mut spine_ids: Vec<Vec<u64>> = Vec::new();
    for (b, branch) in branches.iter().enumerate() {
        let mut ids = Vec::with_capacity(branch.spine.len());
        let mut previous = 0;
        for (k, small) in branch.spine.iter().enumerate() {
            let at = layout.spines[b][k];
            previous = writer.push(small, at, vec![previous], small_icon(small));
            ids.push(previous);
            for (c, cluster) in branch.clusters.iter().enumerate() {
                if cluster.spine_index != k {
                    continue;
                }
                let points = &layout.clusters[b][c];
                let icon = format!("big:{}", cluster.big["name"].as_str().expect("big name"));
                let mut link = writer.push(&cluster.big, points[0], vec![previous], icon.clone());
                for (upgrade, at) in cluster.upgrades.iter().zip(&points[1..]) {
                    link = writer.push(upgrade, *at, vec![link], icon.clone());
                }
            }
        }
        spine_ids.push(ids);
    }
    for (ring, bridge_at) in rings.iter().zip(&layout.bridges) {
        let from = spine_ids[ring.from.0][ring.from.1];
        let to = spine_ids[ring.to.0][ring.to.1];
        match (&ring.bridge, bridge_at) {
            (Some(bridge), Some(at)) => {
                writer.push(bridge, *at, vec![from, to], small_icon(bridge));
            }
            _ => writer
                .nodes
                .get_mut(&from)
                .and_then(|node| node["links"].as_array_mut())
                .expect("links")
                .push(json!(to)),
        }
    }
    writer.finish()
}
