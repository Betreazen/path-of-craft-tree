fn path_of_craft_tree_fx_farm_use_item(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) {
    crate::game::pct_fx_farm_alchemy::on_use_item(game, stack);
}
