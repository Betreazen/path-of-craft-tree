//! Hot paths of the game-free code on the real tree, see docs/perf-spec.md. Frozen once the
//! baseline is taken: change the code under test, never these benchmarks.
use criterion::{Criterion, criterion_group, criterion_main};
use path_of_craft_tree_rules::look::{LinkLook, Look, Shown, Standing};
use path_of_craft_tree_rules::placed::{Placed, RegionKey};
use path_of_craft_tree_rules::rules::{Profile, Tree};
use std::hint::black_box;

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");
/// Plenty for every branch to reach its last level.
const RICH_XP: f64 = 1.0e9;
/// One-time rewards a long-played profile has collected: biomes, structures, crafts.
const SEEN: usize = 300;

/// A long-played player: every branch at its top level and every point spent, nodes taken in
/// id order wherever the rules allow.
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

fn benches(c: &mut Criterion) {
    let tree = Tree::parse(TREE).expect("data/tree.json");
    let look = Look::new(&tree);
    let player = veteran(&tree);
    let effects = tree.effects(&player);
    let (some_branch, other_branch) = {
        let mut ids = tree.branches.keys();
        (ids.next().unwrap().clone(), ids.next().unwrap().clone())
    };

    c.bench_function("effects_clone", |b| b.iter(|| black_box(&effects).clone()));
    c.bench_function("tree_effects", |b| {
        b.iter(|| tree.effects(black_box(&player)))
    });
    c.bench_function("award", |b| {
        b.iter(|| {
            let main = tree.award_main(black_box(&player), &some_branch, 1.0);
            tree.award_cross(&main, &other_branch, 0.5)
        })
    });
    // Added with hypothesis 3: the in-place path grant() now takes, next to the frozen `award`.
    c.bench_function("award_in_place", |b| {
        let mut profile = player.clone();
        b.iter(|| {
            tree.add_main_xp(black_box(&mut profile), &some_branch, 1.0);
            tree.add_cross_xp(&mut profile, &other_branch, 0.5);
            let before = profile.xp.clone();
            tree.levels_changed(&before, &profile.xp)
        })
    });
    c.bench_function("standing_frame", |b| {
        b.iter(|| {
            let standing = Standing::new(&tree, black_box(&player));
            look.draw_order
                .iter()
                .map(|&id| standing.state(id).index())
                .sum::<usize>()
        })
    });
    // Added with the second round: the tree window's own work for one frame with the whole tree
    // in view, without the game's drawing calls. The client's profile has no one-time rewards.
    let mut on_client = player.clone();
    on_client.seen.clear();
    // Before: a copy of the profile and a fresh `Standing` every frame, the energy's links
    // picked from all links, the header's points counted again.
    c.bench_function("window_frame_standing", |b| {
        b.iter(|| {
            let profile = black_box(&on_client).clone();
            let standing = Standing::new(&tree, &profile);
            let states: usize = look
                .draw_order
                .iter()
                .map(|&id| standing.state(id).index())
                .sum();
            let links = look
                .edges
                .iter()
                .filter(|(a, b)| standing.link(*a, *b) != LinkLook::Plain)
                .count();
            let flow: f32 = look
                .edges
                .iter()
                .filter(|(a, b)| standing.flows(*a, *b))
                .map(|(a, b)| {
                    let (da, db) = (look.dist[a], look.dist[b]);
                    let (p, q) = (&tree.nodes[a], &tree.nodes[b]);
                    da.min(db) + (q.x - p.x).hypot(q.y - p.y)
                })
                .sum();
            let points = tree.general_points(&profile) as usize;
            states + links + points + flow as usize
        })
    });
    // After: `Shown` is worked out once per profile message; a frame only reads it.
    let shown = Shown::new(&tree, &look, &on_client);
    c.bench_function("window_frame_shown", |b| {
        b.iter(|| {
            let shown = black_box(&shown);
            let states: usize = look
                .draw_order
                .iter()
                .map(|&id| shown.state(id).index())
                .sum();
            let links = shown
                .links
                .iter()
                .filter(|link| **link != LinkLook::Plain)
                .count();
            let flow: f32 = shown.flows.iter().map(|f| f.start + f.length).sum();
            states + links + shown.general_points as usize + flow as usize
        })
    });
    c.bench_function("shown_new", |b| {
        b.iter(|| Shown::new(&tree, &look, black_box(&on_client)))
    });
    // Added with the second round: what the read ahead of placed marks adds to every player
    // tick once the regions around the player are loaded.
    c.bench_function("prefetch_check", |b| {
        let mut placed = Placed::default();
        let near = |at| RegionKey::near("minecraft:overworld", at, 160);
        for key in near((5, 64, 5)) {
            placed.claim(&key);
            placed.merge(key, Some(Vec::new()));
        }
        b.iter(|| {
            near(black_box((5, 64, 5)))
                .iter()
                .filter(|key| placed.claim(key))
                .count()
        })
    });
    c.bench_function("sanitize", |b| b.iter(|| tree.sanitize(black_box(&player))));
    // Added with hypothesis 5: the window's layout, worked out on its first opening.
    c.bench_function("look_new", |b| b.iter(|| Look::new(black_box(&tree))));
    c.bench_function("tree_parse", |b| b.iter(|| Tree::parse(black_box(TREE))));
    c.bench_function("profile_json", |b| {
        b.iter(|| serde_json::to_vec(black_box(&player)).map(|bytes| bytes.len()))
    });
}

criterion_group!(rules, benches);
criterion_main!(rules);
