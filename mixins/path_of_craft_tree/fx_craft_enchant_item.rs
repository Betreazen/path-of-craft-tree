fn path_of_craft_tree_fx_craft_enchant_item(game: &mut crate::game::Game<'_>, player: u128, item: &horizon_client::items::ItemStack, levels: i32) {
    crate::game::pct_fx_craft_enchant::on_enchant_item(game, player, item, levels);
}
