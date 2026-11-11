fn path_of_craft_tree_player_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::pct_glue::on_logout(game, player);
}
