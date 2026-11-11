fn path_of_craft_tree_player_changed_dimension(game: &mut crate::game::Game<'_>, player: u128, to: &str) {
    crate::game::pct_actions::on_dimension_changed(game, player, to);
}
