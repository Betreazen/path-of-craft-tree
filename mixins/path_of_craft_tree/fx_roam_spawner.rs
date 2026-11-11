fn path_of_craft_tree_fx_roam_spawner(value: HashMap<u32, u32>) -> HashMap<u32, u32> {
    crate::game::pct_fx_roam_combat::mark_spawner_mobs(&value);
    value
}
