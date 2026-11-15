fn path_of_craft_tree_fx_roam_spawner(value: HashMap<u32, u32>, game: &mut crate::game::Game<'_>) -> HashMap<u32, u32> {
    crate::game::pct_fx_roam_combat::mark_spawner_mobs(game, &value);
    value
}
