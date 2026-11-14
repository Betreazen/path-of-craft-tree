//! The tree window's search, planner and share strings on the real tree (docs/perf-spec.md,
//! path C1). A frame only reads what these work out: the search once per query and profile,
//! the plan's view once per plan and profile. `window_frame_marks` is the part a frame does.
use criterion::{Criterion, criterion_group, criterion_main};
use path_of_craft_tree_rules::find::Index;
use path_of_craft_tree_rules::look::{Look, Shown};
use path_of_craft_tree_rules::plan::{self, Plan};
use path_of_craft_tree_rules::rules::{PROFILE_LAYOUT, Profile, Tree};
use std::hint::black_box;

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");

/// Every node the rules allow at this experience, taken in id order.
fn grown(tree: &Tree, xp: f64) -> Profile {
    let mut profile = tree.new_profile();
    for branch in tree.branches.keys() {
        profile.xp.insert(branch.clone(), xp);
    }
    profile.general_xp = xp;
    while let Some(next) = tree
        .nodes
        .keys()
        .find_map(|&id| tree.allocate(&profile, id).ok())
    {
        profile = next;
    }
    profile
}

fn benches(c: &mut Criterion) {
    let tree = Tree::parse(TREE).expect("data/tree.json");
    let look = Look::new(&tree);
    let veteran = grown(&tree, 1.0e9);
    let middle = grown(&tree, 3_000.0);
    let shown = Shown::new(&tree, &look, &middle);
    let index = Index::new(&tree, &look);
    let build = plan::learned_steps(&tree, &veteran);
    let (wanted, _) = Plan::fitted(&tree, &middle, &build);
    let text = plan::encode(PROFILE_LAYOUT, &build);

    c.bench_function("search_index_new", |b| {
        b.iter(|| Index::new(black_box(&tree), &look))
    });
    c.bench_function("search_query", |b| {
        b.iter(|| index.find(black_box("урон"), false, &shown))
    });
    c.bench_function("search_available", |b| {
        b.iter(|| index.find(black_box(""), true, &shown))
    });
    c.bench_function("plan_cost", |b| {
        b.iter(|| black_box(&wanted).cost(&tree, &middle))
    });
    c.bench_function("plan_refit", |b| {
        b.iter(|| black_box(&wanted).refit(&tree, &middle))
    });
    c.bench_function("share_encode", |b| {
        b.iter(|| plan::encode(PROFILE_LAYOUT, black_box(&build)))
    });
    c.bench_function("share_decode", |b| {
        b.iter(|| plan::decode(black_box(&text), &tree, PROFILE_LAYOUT))
    });
    // What a frame does with a search on: the mask looked up for every node in draw order.
    let found = index.find("урон", false, &shown);
    c.bench_function("window_frame_marks", |b| {
        b.iter(|| {
            look.draw_order
                .iter()
                .filter(|id| black_box(&found).contains(**id))
                .count()
        })
    });
}

criterion_group!(window, benches);
criterion_main!(window);
