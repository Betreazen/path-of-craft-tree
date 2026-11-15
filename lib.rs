// Builds the game-free modules of the mod on their own, so `cargo test` runs in seconds.
#[path = "mixins/path_of_craft_tree/blueprints.rs"]
mod blueprints;

#[path = "mixins/path_of_craft_tree/board_rules.rs"]
pub mod board_rules;
#[path = "mixins/path_of_craft_tree/command_rules.rs"]
pub mod command_rules;
#[path = "mixins/path_of_craft_tree/effect_rules.rs"]
mod effect_rules;
#[path = "mixins/path_of_craft_tree/find.rs"]
pub mod find;
#[path = "mixins/path_of_craft_tree/fx_craft_rules.rs"]
mod fx_craft_rules;
#[path = "mixins/path_of_craft_tree/fx_farm_rules.rs"]
mod fx_farm_rules;
#[path = "mixins/path_of_craft_tree/fx_gather_rules.rs"]
mod fx_gather_rules;
#[path = "mixins/path_of_craft_tree/fx_roam_rules.rs"]
mod fx_roam_rules;
#[path = "mixins/path_of_craft_tree/lang.rs"]
pub mod lang;
#[path = "mixins/path_of_craft_tree/limits.rs"]
pub mod limits;

#[path = "mixins/path_of_craft_tree/group_rules.rs"]
pub mod group_rules;
#[path = "mixins/path_of_craft_tree/look.rs"]
pub mod look;
#[path = "mixins/path_of_craft_tree/machine_rules.rs"]
pub mod machine_rules;
#[path = "mixins/path_of_craft_tree/pace_counter.rs"]
pub mod pace_counter;
#[path = "mixins/path_of_craft_tree/placed.rs"]
pub mod placed;
#[path = "mixins/path_of_craft_tree/plan.rs"]
pub mod plan;
#[path = "mixins/path_of_craft_tree/rules.rs"]
pub mod rules;
#[path = "mixins/path_of_craft_tree/session.rs"]
pub mod session;

#[path = "mixins/path_of_craft_tree/settings_rules.rs"]
pub mod settings_rules;
#[path = "mixins/path_of_craft_tree/view.rs"]
pub mod view;
#[path = "mixins/path_of_craft_tree/visits.rs"]
pub mod visits;

/// The paths the mod's modules use inside the game.
pub mod game {
    pub use crate::look as pct_look;
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

    fn english() -> crate::lang::Dict {
        crate::lang::english(&[
            include_bytes!("mixins/path_of_craft_tree/data/en-names.json"),
            include_bytes!("mixins/path_of_craft_tree/data/en-texts.json"),
            include_bytes!("mixins/path_of_craft_tree/data/en-ui.json"),
        ])
        .expect("data/en-*.json")
    }

    fn is_russian(text: &str) -> bool {
        text.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё'))
    }

    /// A player whose game is not in Russian reads the tree in English: every text the window
    /// shows has its English, and none of it is Russian any more.
    #[test]
    fn every_text_of_the_shipped_tree_has_its_english() {
        let tree = shipped();
        let en = english();
        let mut missing: Vec<String> = Vec::new();
        let mut check = |text: &str| {
            if is_russian(text) && en.english(text).is_none() {
                missing.push(text.to_owned());
            }
        };
        for branch in tree.branches.values() {
            check(&branch.name);
            branch.legend.as_deref().map(&mut check);
        }
        for node in tree.nodes.values() {
            check(&node.name);
            check(&node.description);
            node.rank_descriptions.iter().for_each(|text| check(text));
        }
        for title in tree.mastery.iter().flat_map(|m| &m.titles) {
            check(title);
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "no English for {missing:?}");
        let translated = tree.translated(|ru| en.english(ru).unwrap_or_else(|| ru.to_owned()));
        let russian_left = translated
            .nodes
            .values()
            .filter(|node| is_russian(&node.name) || is_russian(&node.description))
            .count();
        assert_eq!(russian_left, 0);
        assert_eq!(translated.nodes.len(), tree.nodes.len());
        assert_eq!(translated.nodes[&0].name, "Start");
    }

    #[test]
    fn the_shipped_tree_has_all_sixteen_branches() {
        let tree = shipped();
        assert_eq!(tree.branches.len(), 16);
        let count = |kind| tree.nodes.values().filter(|node| node.kind == kind).count();
        assert_eq!(count(Kind::Big), 160);
        assert_eq!(count(Kind::Small), 646);
        assert_eq!(count(Kind::Upgrade), 393);
        assert_eq!(tree.nodes.len(), 1200);
    }

    /// The owner's rule (2026-11-15): a branch's 75 levels pay 75 points and its nodes cost more,
    /// so a player chooses; general points can fill a branch, but not all of them.
    #[test]
    fn every_branch_costs_more_than_its_own_points() {
        let tree = shipped();
        let mut whole_tree = 0;
        for id in tree.branches.keys() {
            let points: u32 = tree
                .nodes
                .values()
                .filter(|n| n.branch.as_deref() == Some(id))
                .map(|n| u32::from(n.max_rank()))
                .sum();
            let levels = tree.curve.max_level;
            assert!(points > levels, "{id} costs only {points} points");
            assert!(points <= levels + 15, "{id} costs {points} points");
            whole_tree += points;
        }
        let every_level = tree.curve.max_level * (tree.branches.len() as u32 + 1);
        assert!(
            whole_tree > every_level,
            "{whole_tree} points buy the whole tree"
        );
    }

    /// The nodes saved profiles know: ids below this kept their meaning when nodes were added.
    const OLD_NODES: u16 = 995;

    /// FNV-1a over what each old node is: its name, kind, branch, ranks, effects and its links to
    /// other old nodes.
    fn old_numbering_checksum(tree: &Tree) -> u64 {
        tree.nodes
            .values()
            .filter(|node| node.id < OLD_NODES)
            .map(|node| {
                let mut links: Vec<u16> = node
                    .links
                    .iter()
                    .copied()
                    .filter(|link| *link < OLD_NODES)
                    .collect();
                links.sort_unstable();
                format!(
                    "{}|{}|{:?}|{:?}|{}|{:?}|{links:?}\n",
                    node.id, node.name, node.kind, node.branch, node.ranks, node.effects
                )
            })
            .flat_map(String::into_bytes)
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            })
    }

    /// Taken from the 995-node tree of 0.4.11, the last one before nodes were appended.
    const OLD_NUMBERING_CHECKSUM: u64 = 0xdffe_0a03_bc42_40b9;

    #[test]
    fn appended_nodes_keep_the_old_numbering() {
        let tree = shipped();
        assert_eq!(
            old_numbering_checksum(&tree),
            OLD_NUMBERING_CHECKSUM,
            "now {:#x}",
            old_numbering_checksum(&tree)
        );
        // Every newer node is a booster after the last upgrade of an old chain or of another new
        // booster, so nothing old sits behind a node an old profile cannot have.
        for node in tree.nodes.values().filter(|n| n.id >= OLD_NODES) {
            assert_eq!(node.kind, Kind::Upgrade, "node {}", node.id);
            assert!(!node.effects.is_empty() && !node.todo, "node {}", node.id);
            let behind: Vec<u16> = node
                .links
                .iter()
                .copied()
                .filter(|l| *l > node.id)
                .collect();
            assert!(behind.len() <= 1, "node {} opens {behind:?}", node.id);
            assert!(behind.iter().all(|l| *l >= OLD_NODES), "node {}", node.id);
        }
    }

    #[test]
    fn a_profile_saved_with_every_old_node_keeps_them_all() {
        let tree = shipped();
        let mut saved = tree.new_profile();
        for (i, branch) in tree.branches.keys().enumerate() {
            saved.xp.insert(branch.clone(), 1.0e9 + i as f64);
        }
        saved.general_xp = 1.0e9;
        for node in tree.nodes.values().filter(|n| n.id < OLD_NODES) {
            saved.ranks.insert(node.id, node.max_rank());
        }
        saved.title = Some("mining".to_owned());
        let bytes = serde_json::to_vec(&saved).unwrap();
        let (loaded, dropped) = tree.load_saved(&bytes).unwrap();
        assert!(!dropped);
        assert_eq!(loaded.ranks, saved.ranks);
        assert_eq!(loaded.title, saved.title);
    }

    /// The experience a branch has at the end of `level`.
    fn xp_at_level(tree: &Tree, level: u32) -> f64 {
        (1..=level).map(|l| tree.xp_for_level(l)).sum()
    }

    #[test]
    fn the_shipped_mastery_is_slow_capped_and_titled_per_branch() {
        let tree = shipped();
        let cap = xp_at_level(&tree, tree.curve.max_level);
        assert_eq!(cap, 178_264.0);
        assert_eq!(tree.max_mastery(), 10);
        let rank_at = |xp: f64| tree.mastery_progress(xp).0;
        assert_eq!(rank_at(cap - 1.0), 0);
        assert_eq!(rank_at(cap), 0);
        assert_eq!(rank_at(cap + 5_999.0), 0);
        assert_eq!(rank_at(cap + 6_000.0), 1);
        // The ranks need floor(6000 · r^1.25) each: all ten about 528 000 more experience, about
        // 148 hours at the 3 565 an hour the pace aims for, each a little longer than the last.
        let steps: Vec<f64> = (1..=10u32)
            .map(|r| (6000.0 * f64::from(r).powf(1.25)).floor())
            .collect();
        assert!(steps.windows(2).all(|w| w[1] > w[0]));
        let all: f64 = steps.iter().sum();
        assert_eq!(rank_at(cap + all - 1.0), 9);
        assert_eq!(rank_at(cap + all), 10);
        assert_eq!(tree.mastery_progress(cap + all * 10.0), (10, 1.0));
        assert!((500_000.0..560_000.0).contains(&all), "{all}");
        for (id, branch) in &tree.branches {
            let legend = branch.legend.as_deref().expect("every branch has a legend");
            assert_eq!(tree.mastery_title(id, 10).as_deref(), Some(legend));
            for rank in 1..10 {
                let title = tree.mastery_title(id, rank).expect("a title per rank");
                assert_ne!(title, legend, "{id} {rank}");
            }
        }
        // Mastery pays no points: the branch's own points stop at its last level.
        let mut profile = tree.new_profile();
        profile.xp.insert("mining".to_owned(), cap + all);
        assert_eq!(tree.mastery_rank(&profile, "mining"), 10);
        assert_eq!(tree.branch_points(&profile, "mining"), tree.curve.max_level);
        let shown = tree.shown_title(&profile).expect("a title");
        assert_eq!((shown.branch.as_str(), shown.rank), ("mining", 10));
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
        kept.placed_falling = saved.placed_falling.clone();
        kept.title = saved.title.clone();
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
    /// Retaken 2026-11-15 for the 1200-node tree: the spines are as approved, the booster chains
    /// are longer and the side clusters were laid out again.
    const ZONE_CHECKSUM: u64 = 0x302e_2280_c568_0def;

    /// A link's look the way paint.rs worked it out every frame from a fresh `Standing`.
    fn link_by_standing(s: &crate::look::Standing, a: u16, b: u16) -> crate::look::LinkLook {
        use crate::look::LinkLook;
        let (fa, fb) = (s.is_full(a), s.is_full(b));
        if fa && fb {
            if s.is_off(a) || s.is_off(b) {
                LinkLook::Off
            } else {
                LinkLook::Learned
            }
        } else if (fa && s.is_open(b)) || (fb && s.is_open(a)) {
            LinkLook::Open
        } else {
            LinkLook::Plain
        }
    }

    /// The energy's links the way paint.rs picked them every frame.
    fn flows_by_standing(
        tree: &Tree,
        look: &crate::look::Look,
        s: &crate::look::Standing,
    ) -> Vec<crate::look::Flow> {
        let mut flows = Vec::new();
        for (edge, (a, b)) in look.edges.iter().enumerate() {
            if !s.is_full(*a) || !s.is_full(*b) || s.is_off(*a) || s.is_off(*b) {
                continue;
            }
            let (da, db) = (look.dist[a], look.dist[b]);
            let (from, to) = if da <= db { (a, b) } else { (b, a) };
            let (p, q) = (&tree.nodes[from], &tree.nodes[to]);
            flows.push(crate::look::Flow {
                edge,
                from: *from,
                to: *to,
                start: da.min(db),
                length: (q.x - p.x).hypot(q.y - p.y),
            });
        }
        flows
    }

    #[test]
    fn the_window_s_view_per_profile_matches_a_standing_per_frame() {
        use crate::look::{LinkLook, Look, Shown, Standing};
        let tree = shipped();
        let look = Look::new(&tree);
        let full = grown(&tree, 4_000.0);
        let mut switched = full.clone();
        switched.disabled = full.ranks.keys().copied().step_by(7).collect();
        let profiles = [tree.new_profile(), grown(&tree, 300.0), full, switched];
        let mut seen_links = std::collections::BTreeSet::new();
        for profile in &profiles {
            let shown = Shown::new(&tree, &look, profile);
            let standing = Standing::new(&tree, profile);
            for &id in tree.nodes.keys() {
                assert_eq!(shown.state(id), standing.state(id), "node {id}");
                assert_eq!(shown.rank(id), standing.rank(id), "node {id}");
            }
            for (edge, (a, b)) in look.edges.iter().enumerate() {
                let old = link_by_standing(&standing, *a, *b);
                assert_eq!(shown.links[edge], old, "link {a}-{b}");
                seen_links.insert(format!("{old:?}"));
            }
            assert_eq!(shown.flows, flows_by_standing(&tree, &look, &standing));
            assert_eq!(shown.general_level, tree.general_level(profile));
            assert_eq!(shown.general_points, tree.general_points(profile));
            for id in tree.branches.keys() {
                let xp = profile.xp.get(id).copied().unwrap_or(0.0);
                let branch = shown.branches[id];
                assert_eq!((branch.level, branch.progress), tree.level_progress(xp));
                assert_eq!(branch.points, tree.branch_points(profile, id));
            }
        }
        let every = [
            LinkLook::Plain,
            LinkLook::Open,
            LinkLook::Learned,
            LinkLook::Off,
        ];
        for kind in every {
            assert!(
                seen_links.contains(&format!("{kind:?}")),
                "{kind:?} not tried"
            );
        }
    }

    #[test]
    fn the_window_s_cull_data_follows_the_tree() {
        let tree = shipped();
        let look = crate::look::Look::new(&tree);
        assert_eq!(look.draw_spots.len(), look.draw_order.len());
        for (id, spot) in look.draw_order.iter().zip(&look.draw_spots) {
            assert_eq!(*spot, [tree.nodes[id].x, tree.nodes[id].y]);
        }
        assert_eq!(look.edge_boxes.len(), look.edges.len());
        for ((a, b), bounds) in look.edges.iter().zip(&look.edge_boxes) {
            for node in [&tree.nodes[a], &tree.nodes[b]] {
                assert!(bounds[0] <= node.x && node.x <= bounds[2]);
                assert!(bounds[1] <= node.y && node.y <= bounds[3]);
            }
        }
    }

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
        assert_eq!(ranked.len(), 121);
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
    fn placed_sand_and_gravel_stay_in_the_saved_profile_only() {
        let tree = shipped();
        let mut profile = grown(&tree, 4000.0);
        profile
            .placed_falling
            .insert("minecraft:sand".to_owned(), 3);
        let saved = serde_json::to_vec(&profile).unwrap();
        let (read, _) = crate::rules::Profile::from_saved(&saved).unwrap();
        assert_eq!(read.placed_falling["minecraft:sand"], 3);
        assert_eq!(tree.sanitize(&read).placed_falling, read.placed_falling);
        assert_eq!(tree.respec(&read).placed_falling, read.placed_falling);
        let client = serde_json::to_string(&read.for_client()).unwrap();
        assert!(!client.contains("placed_falling"), "{client}");
        let empty = serde_json::to_string(&tree.new_profile()).unwrap();
        assert!(!empty.contains("placed_falling"), "{empty}");
    }

    #[test]
    fn a_new_player_s_combat_summary_is_empty_with_the_branch_sizes() {
        let tree = shipped();
        let summary = tree.combat_summary(&tree.new_profile());
        for branch in crate::rules::COMBAT_BRANCHES {
            let all: u32 = tree
                .nodes
                .values()
                .filter(|n| n.branch.as_deref() == Some(branch))
                .map(|n| u32::from(n.ranks))
                .sum();
            assert!(all > 0, "{branch} has no nodes");
            assert_eq!(
                summary.levels[branch],
                [0, tree.curve.max_level],
                "{branch}"
            );
            assert_eq!(summary.ranks[branch], [0, all], "{branch}");
        }
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["v"], 1);
        assert_eq!(json["levels"]["melee"][1], tree.curve.max_level);
        assert_eq!(
            json.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["all_levels", "all_ranks", "general", "levels", "ranks", "v"]
        );
    }

    #[test]
    fn the_combat_summary_counts_working_ranks_and_levels() {
        let tree = shipped();
        let mut profile = grown(&tree, 4000.0);
        let learned =
            |profile: &crate::rules::Profile, branch: &str, off: &crate::rules::Allocated| {
                tree.nodes
                    .values()
                    .filter(|n| n.branch.as_deref() == Some(branch) && !off.contains(&n.id))
                    .map(|n| u32::from(profile.rank(n.id)))
                    .sum::<u32>()
            };
        let none = crate::rules::Allocated::new();
        let summary = tree.combat_summary(&profile);
        for branch in crate::rules::COMBAT_BRANCHES {
            assert_eq!(
                summary.ranks[branch][0],
                learned(&profile, branch, &none),
                "{branch}"
            );
            assert!(
                summary.ranks[branch][0] > 0,
                "{branch} learned nothing in the test"
            );
            assert_eq!(
                summary.levels[branch][0],
                tree.branch_level(&profile, branch)
            );
        }
        // A switched-off big node takes itself and its upgrades out of the count.
        let big = tree
            .nodes
            .values()
            .find(|n| {
                n.branch.as_deref() == Some("melee")
                    && n.kind == Kind::Big
                    && profile.is_learned(n.id)
            })
            .expect("a learned big melee node")
            .id;
        profile = tree.toggle(&profile, big).expect("a big node toggles");
        let off = tree.switched_off(&profile);
        let after = tree.combat_summary(&profile);
        assert_eq!(after.ranks["melee"][0], learned(&profile, "melee", &off));
        assert!(after.ranks["melee"][0] < summary.ranks["melee"][0]);
        assert_eq!(after.ranks["melee"][1], summary.ranks["melee"][1]);
    }

    #[test]
    fn only_metal_gear_and_better_pays_the_smith() {
        let tree = shipped();
        for item in [
            "iron_shovel",
            "golden_sword",
            "copper_pickaxe",
            "chainmail_helmet",
            "diamond_axe",
            "netherite_boots",
        ] {
            let id = format!("minecraft:{item}");
            assert!(
                tree.has_suffix_in("gear_suffixes", &id),
                "{item} pays nothing"
            );
        }
        for item in [
            "wooden_shovel",
            "stone_sword",
            "stone_axe",
            "leather_helmet",
            "turtle_helmet",
            "wooden_pickaxe",
        ] {
            let id = format!("minecraft:{item}");
            assert!(!tree.has_suffix_in("gear_suffixes", &id), "{item} pays");
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

// The server owner's settings, admin commands, the leaderboard and the wider summary, on the
// shipped tree.
#[cfg(test)]
mod server_slice {
    use crate::board_rules::{self, Board};
    use crate::command_rules::{self, Admin};
    use crate::rules::{Profile, Tree, XpScale};
    use std::collections::BTreeMap;

    fn shipped() -> Tree {
        Tree::parse(include_bytes!("mixins/path_of_craft_tree/data/tree.json"))
            .expect("data/tree.json")
    }

    fn earned(tree: &Tree) -> Profile {
        let mut profile = tree.new_profile();
        tree.add_main_xp(&mut profile, "mining", 1234.5);
        tree.add_cross_xp(&mut profile, "smithing", 77.25);
        tree.add_main_xp(&mut profile, "melee", 0.1);
        profile
    }

    #[test]
    fn the_default_scale_pays_exactly_what_the_tree_says() {
        let tree = shipped();
        assert_eq!(tree.scale, XpScale::default());
        let profile = earned(&tree);
        assert_eq!(profile.xp["mining"], 1234.5);
        assert_eq!(profile.xp["smithing"], 77.25 * tree.cross_share);
        assert_eq!(profile.xp["melee"], 0.1);
        let general = 1234.5 * tree.general_share + 0.1 * tree.general_share;
        assert_eq!(profile.general_xp, general);
        let same = tree.with_scale(XpScale::default());
        assert_eq!(earned(&same), profile);
    }

    #[test]
    fn multipliers_scale_main_cross_and_general_experience() {
        let tree = shipped().with_scale(XpScale {
            global: 2.0,
            branches: BTreeMap::from([("mining".to_owned(), 1.5), ("smithing".to_owned(), 0.0)]),
            general_share: Some(0.5),
        });
        let mut profile = tree.new_profile();
        tree.add_main_xp(&mut profile, "mining", 10.0);
        tree.add_cross_xp(&mut profile, "smithing", 10.0);
        tree.add_cross_xp(&mut profile, "crafting", 10.0);
        assert_eq!(profile.xp["mining"], 30.0);
        assert_eq!(profile.xp["smithing"], 0.0);
        assert_eq!(profile.xp["crafting"], 10.0 * tree.cross_share * 2.0);
        // The general scale gets its share of the multiplied main experience only.
        assert_eq!(profile.general_xp, 15.0);
        assert_eq!(tree.general_share_in_use(), 0.5);
    }

    #[test]
    fn levels_start_where_the_curve_says() {
        let tree = shipped();
        assert_eq!(tree.xp_at_level(0), 0.0);
        for level in 1..=tree.curve.max_level {
            let xp = tree.xp_at_level(level);
            assert_eq!(tree.level_progress(xp).0, level, "level {level}");
            assert_eq!(tree.level_progress(xp - 0.5).0, level - 1, "level {level}");
        }
        let top = tree.xp_at_level(tree.curve.max_level);
        assert_eq!(tree.xp_at_level(tree.curve.max_level + 5), top);
    }

    #[test]
    fn the_summary_keeps_its_combat_fields_and_adds_every_branch() {
        let tree = shipped();
        let mut profile = tree.new_profile();
        profile.xp.insert("mining".to_owned(), tree.xp_at_level(12));
        profile.xp.insert("melee".to_owned(), tree.xp_at_level(3));
        profile.general_xp = tree.xp_at_level(2);
        let summary = tree.combat_summary(&profile);
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["v"], 1);
        let keys = |field: &str| {
            json[field]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(keys("levels"), ["archery", "defense", "melee"]);
        assert_eq!(keys("ranks"), ["archery", "defense", "melee"]);
        assert_eq!(json["levels"]["melee"][0], 3);
        let max = tree.curve.max_level;
        assert_eq!(summary.all_levels.len(), 16);
        assert_eq!(summary.all_ranks.len(), 16);
        assert_eq!(summary.all_levels["mining"], [12, max]);
        assert_eq!(summary.all_levels["fishing"], [0, max]);
        assert_eq!(summary.general, [2, max]);
        for branch in crate::rules::COMBAT_BRANCHES {
            assert_eq!(summary.all_levels[branch], summary.levels[branch]);
            assert_eq!(summary.all_ranks[branch], summary.ranks[branch]);
        }
        for (branch, [learned, all]) in &summary.all_ranks {
            let total: u32 = tree
                .nodes
                .values()
                .filter(|n| n.branch.as_deref() == Some(branch.as_str()))
                .map(|n| u32::from(n.ranks))
                .sum();
            assert_eq!((*learned, *all), (0, total), "{branch}");
        }
    }

    fn xp(branch: &str, amount: f64) -> Admin {
        Admin::Xp {
            player: "x".to_owned(),
            branch: branch.to_owned(),
            amount,
        }
    }

    fn level(branch: &str, level: u32) -> Admin {
        Admin::Level {
            player: "x".to_owned(),
            branch: branch.to_owned(),
            level,
        }
    }

    #[test]
    fn admin_xp_gives_takes_and_never_goes_below_zero() {
        let tree = shipped();
        let give = |profile: &Profile, branch: &str, amount: f64| {
            command_rules::apply(&tree, profile, &xp(branch, amount))
        };
        let profile = give(&tree.new_profile(), "mining", 500.0).unwrap();
        assert_eq!(profile.xp["mining"], 500.0);
        // Exact numbers: no multiplier and no general share.
        assert_eq!(profile.general_xp, 0.0);
        let profile = give(&profile, "mining", -200.0).unwrap();
        assert_eq!(profile.xp["mining"], 300.0);
        let profile = give(&profile, "mining", -1000.0).unwrap();
        assert_eq!(profile.xp["mining"], 0.0);
        let profile = give(&profile, "general", 40.0).unwrap();
        assert_eq!(profile.general_xp, 40.0);
        assert!(give(&profile, "minign", 1.0).is_err());
    }

    #[test]
    fn admin_level_sets_the_level_and_taking_levels_gives_back_nodes() {
        let tree = shipped();
        let set = |profile: &Profile, branch: &str, to: u32| {
            command_rules::apply(&tree, profile, &level(branch, to))
        };
        let profile = set(&tree.new_profile(), "mining", 20).unwrap();
        assert_eq!(tree.branch_level(&profile, "mining"), 20);
        let profile = set(&profile, "general", 4).unwrap();
        assert_eq!(tree.general_level(&profile), 4);
        assert!(set(&profile, "mining", tree.curve.max_level + 1).is_err());
        // Learn what the points pay for, then take the levels away: the nodes go.
        let mut learned = profile.clone();
        while let Some(node) = tree
            .nodes
            .values()
            .find(|n| tree.can_allocate(&learned, n.id).is_ok())
        {
            learned = tree.allocate(&learned, node.id).unwrap();
        }
        assert!(learned.ranks.len() > 1, "nothing learned");
        let lowered = set(&set(&learned, "mining", 0).unwrap(), "general", 0).unwrap();
        assert_eq!(lowered.ranks, tree.new_profile().ranks);
    }

    #[test]
    fn admin_reset_refunds_every_node_and_keeps_the_experience() {
        let tree = shipped();
        let mut profile = tree.new_profile();
        profile.xp.insert("mining".to_owned(), tree.xp_at_level(10));
        let node = tree
            .nodes
            .values()
            .find(|n| tree.can_allocate(&profile, n.id).is_ok())
            .unwrap()
            .id;
        let profile = tree.allocate(&profile, node).unwrap();
        let reset = Admin::Reset {
            player: "x".to_owned(),
        };
        let reset = command_rules::apply(&tree, &profile, &reset).unwrap();
        assert_eq!(reset.ranks, tree.new_profile().ranks);
        assert_eq!(reset.xp, profile.xp);
        let lines = command_rules::describe(&tree, &reset);
        assert_eq!(lines.len(), 18);
        assert!(
            lines.iter().any(|l| l.contains("(mining): уровень 10")),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_leaderboard_row_has_every_branch_and_the_total() {
        let tree = shipped();
        let mut profile = tree.new_profile();
        profile.xp.insert("mining".to_owned(), tree.xp_at_level(7));
        profile.xp.insert("fishing".to_owned(), tree.xp_at_level(5));
        let row = board_rules::row(&tree, &profile, 0xabc, "Steve", 1_700_000_000);
        assert_eq!(row.uuid, "00000000000000000000000000000abc");
        assert_eq!(row.levels.len(), 16);
        assert_eq!(row.levels["mining"], 7);
        assert_eq!(row.total, 12);
        assert_eq!(row.general, 0);
        assert_eq!(row.updated, 1_700_000_000);
    }

    #[test]
    fn the_leaderboard_keeps_one_row_per_player_sorted_by_total() {
        let tree = shipped();
        let with = |mining: u32| {
            let mut p = tree.new_profile();
            p.xp.insert("mining".to_owned(), tree.xp_at_level(mining));
            p
        };
        let mut board = Board::default();
        board.upsert(board_rules::row(&tree, &with(3), 1, "Bea", 10));
        board.upsert(board_rules::row(&tree, &with(9), 2, "Al", 10));
        board.upsert(board_rules::row(&tree, &with(3), 3, "Ann", 10));
        board.upsert(board_rules::row(&tree, &with(5), 1, "Bea", 20));
        assert!(board.has(1));
        assert_eq!(board.players.len(), 3);
        let read = Board::parse(&board.encode(&tree, 30).unwrap()).unwrap();
        let order: Vec<&str> = read.players.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(order, ["Al", "Bea", "Ann"]);
        assert_eq!(read.v, 1);
        assert_eq!(read.updated, 30);
        assert_eq!(read.max_level, tree.curve.max_level);
        assert_eq!(read.branches["mining"], "Шахтёр");
        assert!(board.remove(2));
        assert!(!board.remove(2));
        assert!(!board.has(2));
    }
}
