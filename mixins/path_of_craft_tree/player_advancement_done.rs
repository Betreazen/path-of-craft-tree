fn path_of_craft_tree_player_advancement_done(game: &mut crate::game::Game<'_>, player: u128, advancement: &str) {
    crate::game::pct_actions::on_advancement(game, player, advancement);
}
