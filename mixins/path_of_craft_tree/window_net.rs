fn path_of_craft_tree_window_net(channel: &str, data: &[u8]) {
    crate::game::pct_window::on_server_message(channel, data);
}
