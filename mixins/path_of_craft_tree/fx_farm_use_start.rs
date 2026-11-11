fn path_of_craft_tree_fx_farm_use_start(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    duration: &mut i32,
) {
    crate::game::pct_fx_farm_cooking::on_use_start(game, stack, duration);
}
