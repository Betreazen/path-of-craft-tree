fn path_of_craft_tree_fx_farm_use_item_on(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
) {
    crate::game::pct_fx_farm::on_use_item_on(game, stack, pos, face);
}
