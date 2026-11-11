//! Writes the long-played profile the benchmarks use (benches/rules.rs) as a saved profile, so
//! the in-game perf scenario measures the same heavy player: `cargo run --example veteran OUT`.
use path_of_craft_tree_rules::rules::{Profile, Tree};

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");
const RICH_XP: f64 = 1.0e9;
const SEEN: usize = 300;

fn veteran(tree: &Tree) -> Profile {
    let mut profile = tree.new_profile();
    for branch in tree.branches.keys() {
        profile.xp.insert(branch.clone(), RICH_XP);
    }
    profile.general_xp = RICH_XP;
    profile.seen = (0..SEEN)
        .map(|i| format!("craft:minecraft:item_{i}"))
        .collect();
    loop {
        let mut grown = false;
        for &id in tree.nodes.keys() {
            while let Ok(next) = tree.allocate(&profile, id) {
                profile = next;
                grown = true;
            }
        }
        if !grown {
            return profile;
        }
    }
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).ok_or("usage: veteran OUT")?;
    let tree = Tree::parse(TREE)?;
    let profile = veteran(&tree);
    let effects = tree.effects(&profile);
    let json = serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?;
    std::fs::write(&out, json).map_err(|e| e.to_string())?;
    println!(
        "{} nodes, {} ranks, {} effect keys, {} seen -> {out}",
        profile.ranks.len(),
        profile.ranks.values().map(|r| u32::from(*r)).sum::<u32>(),
        effects.len(),
        profile.seen.len()
    );
    Ok(())
}
