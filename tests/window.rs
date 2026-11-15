//! The tree window's search, planner and share strings on the shipped tree.
use path_of_craft_tree_rules::find::Index;
use path_of_craft_tree_rules::look::{Look, Shown, State};
use path_of_craft_tree_rules::plan::{self, Plan};
use path_of_craft_tree_rules::rules::{PROFILE_LAYOUT, Profile, Tree};

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");

fn shipped() -> Tree {
    Tree::parse(TREE).expect("data/tree.json")
}

/// Every node the rules allow, taken in id order until nothing more can be learned.
fn grown(tree: &Tree, xp_per_branch: f64) -> Profile {
    let mut profile = tree.new_profile();
    for (i, branch) in tree.branches.keys().enumerate() {
        profile
            .xp
            .insert(branch.clone(), xp_per_branch * (i as f64 + 1.0));
    }
    profile.general_xp = xp_per_branch * 8.0;
    while let Some(next) = tree
        .nodes
        .keys()
        .find_map(|&id| tree.allocate(&profile, id).ok())
    {
        profile = next;
    }
    profile
}

#[test]
fn a_whole_build_travels_in_a_share_string_and_replays_exactly() {
    let tree = shipped();
    let built = grown(&tree, 4_000.0);
    let steps = plan::learned_steps(&tree, &built);
    let learned: usize = built.ranks.values().map(|r| usize::from(*r)).sum::<usize>() - 1;
    assert_eq!(steps.len(), learned);
    let text = plan::encode(PROFILE_LAYOUT, &steps);
    // Short enough to paste in chat: about two characters a rank.
    assert!(
        text.len() < steps.len() * 3 + 16,
        "{} for {}",
        text.len(),
        steps.len()
    );
    assert_eq!(
        plan::decode(&text, &tree, PROFILE_LAYOUT),
        Ok(steps.clone())
    );
    // On a fresh profile with the same experience the plan is the whole build, in an order the
    // server accepts rank by rank.
    let mut fresh = built.clone();
    fresh.ranks = tree.new_profile().ranks;
    fresh.disabled.clear();
    let (plan, skipped) = Plan::fitted(&tree, &fresh, &steps);
    assert_eq!(plan.steps().len(), steps.len());
    assert_eq!((skipped.learned, skipped.unreachable), (0, 0));
    let cost = plan.cost(&tree, &fresh);
    assert_eq!(cost.missing, 0);
    let mut replayed = fresh;
    for id in plan.steps() {
        replayed = tree
            .allocate(&replayed, *id)
            .expect("each planned rank is allowed");
    }
    assert_eq!(replayed.ranks, built.ranks);
}

#[test]
fn the_cost_of_a_plan_matches_what_the_server_allows() {
    let tree = shipped();
    let poor = grown(&tree, 300.0);
    let rich = grown(&tree, 4_000.0);
    let wanted = plan::learned_steps(&tree, &rich);
    let (plan, _) = Plan::fitted(&tree, &poor, &wanted);
    assert!(!plan.is_empty());
    let cost = plan.cost(&tree, &poor);
    assert!(cost.missing > 0 && cost.affordable < plan.steps().len());
    let mut profile = poor.clone();
    for id in &plan.steps()[..cost.affordable] {
        profile = tree.allocate(&profile, *id).expect("an affordable rank");
    }
    assert!(
        tree.allocate(&profile, plan.steps()[cost.affordable])
            .is_err()
    );
    let branch_sum: u32 = cost.branches.values().map(|b| b.planned).sum();
    assert_eq!(branch_sum, cost.ranks);
}

#[test]
fn the_filter_finds_exactly_the_nodes_the_window_shows_as_available() {
    let tree = shipped();
    let look = Look::new(&tree);
    let index = Index::new(&tree, &look);
    for profile in [tree.new_profile(), grown(&tree, 300.0)] {
        let shown = Shown::new(&tree, &look, &profile);
        let found = index.find("", true, &shown);
        let available: Vec<u16> = tree
            .nodes
            .keys()
            .copied()
            .filter(|id| shown.state(*id) == State::Available)
            .collect();
        let mut ids = found.ids.clone();
        ids.sort_unstable();
        assert_eq!(ids, available);
    }
}

#[test]
fn a_search_finds_a_node_by_its_name_in_any_case() {
    let tree = shipped();
    let look = Look::new(&tree);
    let index = Index::new(&tree, &look);
    let shown = Shown::new(&tree, &look, &tree.new_profile());
    let node = tree
        .nodes
        .values()
        .find(|n| n.id != tree.start && n.name.chars().count() > 5)
        .unwrap();
    let found = index.find(&node.name.to_uppercase(), false, &shown);
    assert!(found.ids.contains(&node.id), "{} not found", node.name);
    // Nearest the start first.
    let dist: Vec<f32> = found.ids.iter().map(|id| look.dist[id]).collect();
    assert!(dist.windows(2).all(|w| w[0] <= w[1]));
}
