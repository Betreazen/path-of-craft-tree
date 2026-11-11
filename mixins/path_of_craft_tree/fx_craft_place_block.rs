fn path_of_craft_tree_fx_craft_place_block(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], state: horizon_world::registry::BlockStateId) {
    crate::game::pct_fx_craft_build::on_place(game, stack, pos, state);
}
