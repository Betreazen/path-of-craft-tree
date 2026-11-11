fn path_of_craft_tree_fx_farm_item_crafted(
    game: &mut crate::game::Game<'_>,
    player: u128,
    stack: &horizon_client::items::ItemStack,
) {
    crate::game::pct_fx_farm_cooking::on_item_crafted(game, player, stack);
}
