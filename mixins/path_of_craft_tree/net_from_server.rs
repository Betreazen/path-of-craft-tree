fn path_of_craft_tree_net_from_server(channel: &str, data: &[u8]) {
    crate::game::pct_glue::on_server_message(channel, data);
}
