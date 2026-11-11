// Builds the game-free modules of the mod on their own, so `cargo test` runs in seconds.
#[path = "mixins/path_of_craft_tree/effect_rules.rs"]
mod effect_rules;
#[path = "mixins/path_of_craft_tree/fx_craft_rules.rs"]
mod fx_craft_rules;
#[path = "mixins/path_of_craft_tree/fx_farm_rules.rs"]
mod fx_farm_rules;
#[path = "mixins/path_of_craft_tree/fx_gather_rules.rs"]
mod fx_gather_rules;
#[path = "mixins/path_of_craft_tree/fx_roam_rules.rs"]
mod fx_roam_rules;
#[path = "mixins/path_of_craft_tree/look.rs"]
pub mod look;
#[path = "mixins/path_of_craft_tree/machine_rules.rs"]
pub mod machine_rules;
#[path = "mixins/path_of_craft_tree/placed.rs"]
pub mod placed;
#[path = "mixins/path_of_craft_tree/rules.rs"]
pub mod rules;
#[path = "mixins/path_of_craft_tree/view.rs"]
pub mod view;

/// The paths the mod's modules use inside the game.
pub mod game {
    pub use crate::rules as pct_rules;
}

// Lives here because horizon-mixins refuses `include_bytes!` inside a mod's modules.
#[cfg(test)]
mod shipped_tree {
    use crate::rules::{Kind, Tree};

    fn shipped() -> Tree {
        Tree::parse(include_bytes!("mixins/path_of_craft_tree/data/tree.json"))
            .expect("data/tree.json")
    }

    #[test]
    fn the_shipped_tree_has_all_sixteen_branches() {
        let tree = shipped();
        assert_eq!(tree.branches.len(), 16);
        let count = |kind| tree.nodes.values().filter(|node| node.kind == kind).count();
        assert_eq!(count(Kind::Big), 160);
        assert_eq!(count(Kind::Small), 646);
    }

    #[test]
    fn every_branch_can_be_filled_with_its_own_points() {
        let tree = shipped();
        for id in tree.branches.keys() {
            let points: u32 = tree
                .nodes
                .values()
                .filter(|n| n.branch.as_deref() == Some(id))
                .map(|n| u32::from(n.max_rank()))
                .sum();
            assert!(points <= tree.curve.max_level, "{id} needs {points} points");
            assert!(points >= 60, "{id} needs only {points} points");
        }
    }

    /// General points the slow way the rules first counted them, branch by branch.
    fn general_points_by_branch(tree: &Tree, profile: &crate::rules::Profile) -> u32 {
        let spent = |keep: &dyn Fn(&crate::rules::Node) -> bool| -> u32 {
            profile
                .ranks
                .iter()
                .filter(|(id, _)| tree.nodes.get(id).is_some_and(keep))
                .map(|(_, rank)| u32::from(*rank))
                .sum()
        };
        let overflow: u32 = tree
            .branches
            .keys()
            .map(|b| {
                spent(&|n| n.branch.as_deref() == Some(b.as_str()))
                    .saturating_sub(tree.branch_level(profile, b))
            })
            .sum();
        let unbranched = spent(&|n| n.branch.is_none() && n.kind != Kind::Start);
        tree.general_level(profile)
            .saturating_sub(overflow)
            .saturating_sub(unbranched)
    }

    /// The level and progress the slow way the rules first worked them out, level by level.
    fn level_by_level(tree: &Tree, xp: f64) -> (u32, f64) {
        let mut left = xp;
        for level in 0..tree.curve.max_level {
            let need = tree.xp_for_level(level + 1);
            if left < need {
                return (level, left / need);
            }
            left -= need;
        }
        (tree.curve.max_level, 1.0)
    }

    #[test]
    fn looked_up_levels_match_the_level_by_level_count() {
        let tree = shipped();
        let mut edge = 0.0;
        let mut samples = vec![0.0, 0.5, 1.0e12];
        for level in 1..=tree.curve.max_level + 1 {
            edge += tree.xp_for_level(level);
            samples.extend([edge - 0.5, edge, edge + 0.25]);
        }
        for xp in samples {
            assert_eq!(
                tree.level_progress(xp),
                level_by_level(&tree, xp),
                "xp {xp}"
            );
        }
    }

    /// Sanitize the way the rules first did it, asking can_allocate before every rank.
    fn sanitize_rank_by_rank(tree: &Tree, saved: &crate::rules::Profile) -> crate::rules::Profile {
        let mut kept = tree.new_profile();
        kept.xp = saved.xp.clone();
        kept.general_xp = saved.general_xp;
        kept.seen = saved.seen.clone();
        let mut frontier = std::collections::VecDeque::from([tree.start]);
        while let Some(id) = frontier.pop_front() {
            let mut links = tree.nodes[&id].links.clone();
            links.sort_unstable();
            for link in links {
                let (wanted, before) = (saved.rank(link), kept.rank(link));
                while kept.rank(link) < wanted && tree.can_allocate(&kept, link).is_ok() {
                    kept.ranks.insert(link, kept.rank(link) + 1);
                }
                if before == 0 && wanted > 0 && tree.is_full(&kept, link) {
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

    /// Every node the rules allow, taken in id order until nothing more can be learned.
    fn grown(tree: &Tree, xp_per_branch: f64) -> crate::rules::Profile {
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
    fn sanitize_keeps_what_the_rank_by_rank_check_keeps() {
        let tree = shipped();
        let full = grown(&tree, 4_000.0);
        assert_eq!(tree.sanitize(&full), sanitize_rank_by_rank(&tree, &full));
        assert_eq!(tree.sanitize(&full).ranks, full.ranks);

        let mut poorer = full.clone();
        for xp in poorer.xp.values_mut() {
            *xp *= 0.3;
        }
        poorer.general_xp *= 0.3;
        let trimmed = tree.sanitize(&poorer);
        assert_eq!(trimmed, sanitize_rank_by_rank(&tree, &poorer));
        assert!(trimmed.ranks.len() < full.ranks.len());

        let mut switched = full.clone();
        switched.disabled = full.ranks.keys().copied().step_by(7).collect();
        assert_eq!(
            tree.sanitize(&switched),
            sanitize_rank_by_rank(&tree, &switched)
        );
    }

    #[test]
    fn one_pass_point_counts_match_the_branch_by_branch_counts_while_a_player_grows() {
        let tree = shipped();
        let mut profile = tree.new_profile();
        // Uneven experience, so some branches overflow into general points and some do not.
        for (i, branch) in tree.branches.keys().enumerate() {
            profile
                .xp
                .insert(branch.clone(), 2_000.0 * (i as f64 + 1.0));
        }
        profile.general_xp = 30_000.0;
        let mut steps = 0;
        loop {
            let points = tree.points(&profile);
            for branch in tree.branches.keys() {
                assert_eq!(
                    points.branch.get(branch.as_str()).copied().unwrap_or(0),
                    tree.branch_points(&profile, branch)
                );
            }
            assert_eq!(points.general, general_points_by_branch(&tree, &profile));
            assert_eq!(tree.general_points(&profile), points.general);
            let next = tree
                .nodes
                .keys()
                .find_map(|&id| tree.allocate(&profile, id).ok());
            match next {
                Some(next) => profile = next,
                None => break,
            }
            steps += 1;
        }
        assert!(steps > 50, "only {steps} steps");
    }

    /// Distances from the start the way the window first worked them out: Dijkstra picking the
    /// nearest open node by scanning them all.
    fn distances_by_scanning(tree: &Tree) -> std::collections::BTreeMap<u16, f32> {
        let length = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let mut dist: std::collections::BTreeMap<u16, f32> =
            tree.nodes.keys().map(|id| (*id, f32::INFINITY)).collect();
        let mut done = std::collections::BTreeSet::new();
        dist.insert(tree.start, 0.0);
        while let Some((id, here)) = dist
            .iter()
            .filter(|(id, d)| d.is_finite() && !done.contains(*id))
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(id, d)| (*id, *d))
        {
            done.insert(id);
            let node = &tree.nodes[&id];
            for link in &node.links {
                let far = &tree.nodes[link];
                let through = here + length([node.x, node.y], [far.x, far.y]);
                if let Some(d) = dist.get_mut(link) {
                    *d = d.min(through);
                }
            }
        }
        dist
    }

    /// FNV-1a over the zone map, so a change to how zones are worked out shows up.
    fn zone_checksum(zones: &crate::look::Zones) -> u64 {
        zones
            .branch
            .iter()
            .chain(&zones.level)
            .chain(&zones.cols.to_le_bytes())
            .chain(&zones.rows.to_le_bytes())
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
            })
    }

    #[test]
    fn the_shipped_colour_zones_stay_as_drawn() {
        let zones = crate::look::Look::new(&shipped()).zones;
        assert_eq!(
            zone_checksum(&zones),
            ZONE_CHECKSUM,
            "zones now {:#x}",
            zone_checksum(&zones)
        );
    }

    /// Taken from the zone map of the approved design; regenerating data/tree.json changes it.
    const ZONE_CHECKSUM: u64 = 0x5e94_290d_004c_3059;

    #[test]
    fn window_distances_match_the_scanning_search() {
        let tree = shipped();
        assert_eq!(
            crate::look::Look::new(&tree).dist,
            distances_by_scanning(&tree)
        );
    }

    #[test]
    fn the_colour_zones_fit_in_the_free_rows_of_the_atlas() {
        let look = crate::look::Look::new(&shipped());
        let atlas: serde_json::Value =
            serde_json::from_slice(include_bytes!("mixins/path_of_craft_tree/data/atlas.json"))
                .expect("data/atlas.json");
        let free = &atlas["free"];
        let (z, width, rows) = (&look.zones, free[2].as_u64(), free[3].as_u64());
        assert!(z.cols as u64 + 2 <= width.unwrap(), "{} columns", z.cols);
        assert!(z.rows as u64 + 2 <= rows.unwrap(), "{} rows", z.rows);
        assert!(z.level.iter().filter(|level| **level > 0).count() > z.level.len() / 4);
    }

    #[test]
    fn every_branch_has_a_colour_and_ranked_nodes_describe_each_rank() {
        let tree = shipped();
        assert!(
            tree.branches
                .values()
                .all(|b| b.colour.is_some() && b.icon.is_some())
        );
        let ranked: Vec<_> = tree.nodes.values().filter(|n| n.max_rank() > 1).collect();
        assert_eq!(ranked.len(), 83);
        for node in ranked {
            assert_eq!(
                node.rank_descriptions.len(),
                usize::from(node.ranks),
                "{}",
                node.id
            );
        }
    }

    #[test]
    fn every_branch_earns_experience() {
        let tree = shipped();
        for (id, branch) in &tree.branches {
            assert!(!branch.xp.is_empty(), "{id} has no experience sources");
        }
    }

    #[test]
    fn the_whole_tree_is_reachable_from_the_start() {
        let tree = shipped();
        let mut seen = std::collections::BTreeSet::from([tree.start]);
        let mut frontier = vec![tree.start];
        while let Some(id) = frontier.pop() {
            for link in &tree.nodes[&id].links {
                if seen.insert(*link) {
                    frontier.push(*link);
                }
            }
        }
        assert_eq!(seen.len(), tree.nodes.len());
    }
}
