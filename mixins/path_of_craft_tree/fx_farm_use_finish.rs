fn path_of_craft_tree_fx_farm_use_finish(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) {
    crate::game::pct_fx_farm_cooking::on_use_finish(game, stack);
}
