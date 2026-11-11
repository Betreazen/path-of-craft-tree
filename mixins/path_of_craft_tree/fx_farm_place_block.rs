fn path_of_craft_tree_fx_farm_place_block(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) {
    crate::game::pct_fx_farm::on_place_block(game, stack);
}
