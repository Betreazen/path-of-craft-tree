// The woodcutter's nodes that act on a whole tree: Timber, Forester, Generous Crown, Stripper.
use crate::game::{
    Game,
    pct_actions::earn_once,
    pct_fx_gather::{self as gather, Job, KIND_LEAVES, KIND_TIMBER},
    pct_fx_gather_rules as rules,
    pct_mining::{chance, effect},
    pct_rules::Effects,
};
use horizon_client::mod_api::NativeServerPlayer;
use std::collections::{HashSet, VecDeque};

const STRIP_ACTION: &str = "act:strip";
const STRIP_BONUS_XP: u32 = 2;

fn is_natural_leaf(game: &Game<'_>, pos: [i32; 3]) -> bool {
    rules::is_leaf_block(&game.native_block(pos))
        && game.native_block_property(pos, "persistent").as_deref() != Some("true")
        && !gather::is_placed(game, pos)
}

fn scan_limit(limit: usize) -> usize {
    if limit == 0 {
        rules::TREE_SCAN_LIMIT
    } else {
        // One more than can be cut shows whether the tree would be left standing.
        (limit + 1).min(rules::TIMBER_TECH_LIMIT)
    }
}

fn tree_jobs(
    game: &Game<'_>,
    block: &str,
    tree: &rules::Tree,
    limit: usize,
    effects: &Effects,
) -> Vec<Job> {
    let cut = rules::logs_to_break(tree.logs.len(), limit);
    let mut jobs: Vec<Job> = tree
        .logs
        .iter()
        .skip(1)
        .take(cut)
        .map(|pos| Job::Break {
            pos: *pos,
            kind: KIND_TIMBER,
            block: block.to_owned(),
        })
        .collect();
    if !rules::tree_finished(tree.logs.len(), limit) {
        return jobs;
    }
    if effect(effects, "canopy") > 0.0 {
        let leaves = rules::scan_leaves(&tree.logs, rules::LEAF_LIMIT, &|pos| {
            is_natural_leaf(game, pos)
        });
        jobs.extend(leaves.into_iter().map(|pos| Job::Break {
            pos,
            kind: KIND_LEAVES,
            block: game.native_block(pos),
        }));
    }
    if effect(effects, "forester") > 0.0 {
        if let Some(sapling) = rules::sapling_of(block) {
            let spots = rules::plant_spots(&tree.logs, effect(effects, "forester_2x2") > 0.0);
            jobs.extend(spots.into_iter().map(|pos| Job::Plant {
                pos,
                block: sapling.clone(),
                grow: effect(effects, "forester_grow"),
            }));
        }
    }
    jobs
}

/// An axe broke a block: fells the tree, or finishes it if this was its last log.
pub fn start_tree(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    effects: &Effects,
) {
    let limit = rules::timber_limit(&|key: &str| effect(effects, key));
    let wanted = limit > 0 || effect(effects, "canopy") > 0.0 || effect(effects, "forester") > 0.0;
    let block = game.native_block(pos);
    if !wanted || !rules::is_tree_log(&block) || gather::broken_was_placed(game, pos) {
        return;
    }
    let jobs = {
        let view: &Game<'_> = game;
        let is_log = |at: [i32; 3]| view.native_block(at) == block && !gather::is_placed(view, at);
        let is_leaf = |at: [i32; 3]| is_natural_leaf(view, at);
        let tree = rules::scan_tree(pos, scan_limit(limit), &is_log, &is_leaf);
        if !tree.has_leaves {
            return;
        }
        tree_jobs(view, &block, &tree, limit, effects)
    };
    gather::run_chain(game, acting.uuid, jobs);
}

fn holds_axe(game: &Game<'_>) -> bool {
    game.native_held()
        .is_some_and(|(item, _, _)| rules::is_axe(&item))
}

/// The natural logs joined to `origin`, without it.
fn trunk(game: &Game<'_>, origin: [i32; 3], block: &str) -> Vec<[i32; 3]> {
    let mut seen = HashSet::from([origin]);
    let mut frontier = VecDeque::from([origin]);
    let mut logs = Vec::new();
    while let Some(at) = frontier.pop_front() {
        for offset in rules::plane_offsets(1, 1)
            .into_iter()
            .chain([[0, 0, 0]])
            .flat_map(|o| [-1, 0, 1].map(|dy| [o[0], dy, o[2]]))
        {
            let next = rules::add(at, offset);
            if logs.len() >= rules::STRIP_LIMIT {
                return logs;
            }
            if seen.insert(next)
                && game.native_block(next) == block
                && !gather::is_placed(game, next)
            {
                logs.push(next);
                frontier.push_back(next);
            }
        }
    }
    logs
}

fn strip_one(game: &mut Game<'_>, pos: [i32; 3], stripped: &str) -> bool {
    let Some(state) = game.native_default_state(stripped) else {
        return false;
    };
    let axis = game.native_block_property(pos, "axis");
    let dimension = game.native_dimension();
    if !game.native_set_block(dimension, pos, state, gather::SET_BLOCK_FLAGS) {
        return false;
    }
    if let Some(axis) = axis {
        game.native_set_block_property(pos, "axis", &axis);
    }
    true
}

/// Stripper: the rest of the trunk loses its bark too, each log paying wear and experience.
fn strip_trunk(
    game: &mut Game<'_>,
    player: u128,
    origin: [i32; 3],
    block: &str,
    effects: &Effects,
) {
    let Some(stripped) = rules::stripped_of(block) else {
        return;
    };
    for log in trunk(game, origin, block) {
        if !holds_axe(game) {
            break;
        }
        if !strip_one(game, log, &stripped) {
            continue;
        }
        earn_once(game, player, STRIP_ACTION, 1.0);
        game.damage_held(1);
        if chance(effect(effects, "strip_bonus")) {
            game.native_give_experience(player, STRIP_BONUS_XP);
        }
    }
}

/// An axe right-clicked a block; the game itself strips the clicked log afterwards.
pub fn on_axe_use(game: &mut Game<'_>, player: u128, pos: [i32; 3], effects: &Effects) {
    let block = game.native_block(pos);
    if !rules::is_strippable(&block) || gather::is_placed(game, pos) {
        return;
    }
    earn_once(game, player, STRIP_ACTION, 1.0);
    if effect(effects, "strip_all") > 0.0 {
        strip_trunk(game, player, pos, &block, effects);
    }
}
