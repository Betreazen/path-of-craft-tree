fn path_of_craft_tree_fx_craft_net(player: u128, channel: &str, data: &[u8]) {
    crate::game::pct_fx_craft::on_client_message(player, channel, data);
}
