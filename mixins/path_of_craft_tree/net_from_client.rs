fn path_of_craft_tree_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::pct_glue::on_client_message(game, player, channel, data);
}
