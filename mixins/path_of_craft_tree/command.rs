fn path_of_craft_tree_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::pct_glue::on_command(game, command, ran);
}
