fn path_of_craft_tree_console_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
    ran: &mut bool,
) {
    crate::game::pct_machines::on_console_command(game, command, ran);
}
