fn path_of_craft_tree_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::pct_glue::on_key(game, key, pressed);
}
