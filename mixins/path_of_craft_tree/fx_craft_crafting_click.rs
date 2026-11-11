fn path_of_craft_tree_fx_craft_crafting_click(host: &crate::game::lan::LanHost, conn: u32, p: &horizon_protocol::packets::play::ContainerClick) {
    crate::game::pct_fx_craft_tables::on_crafting_click(host, conn, p);
}
