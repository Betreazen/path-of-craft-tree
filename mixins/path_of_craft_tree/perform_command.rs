fn path_of_craft_tree_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::pct_commands::on_player_command(game, player, command, ran);
}
