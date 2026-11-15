fn path_of_craft_tree_window_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::pct_window::on_game_key(game, key, pressed);
}
