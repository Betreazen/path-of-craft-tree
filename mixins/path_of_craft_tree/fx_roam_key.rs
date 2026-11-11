fn path_of_craft_tree_fx_roam_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::pct_fx_roam_world::on_key(game, key, pressed);
}
