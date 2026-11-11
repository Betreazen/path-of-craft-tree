fn path_of_craft_tree_fx_craft_furnace_extract(player: u128, item: &str, count: u32) {
    crate::game::pct_fx_craft_items::on_furnace_extract(player, item, count);
}
