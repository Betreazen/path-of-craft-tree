fn path_of_craft_tree_fx_gather_item_damaged(player: u128, item: &str, amount: &mut i32, cancel: &mut bool) {
    crate::game::pct_fx_gather::on_item_damaged(player, item, amount, cancel);
}
