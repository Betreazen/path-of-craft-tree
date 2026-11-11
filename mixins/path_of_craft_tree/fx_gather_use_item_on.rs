fn path_of_craft_tree_fx_gather_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    crate::game::pct_fx_gather_dig::on_use_item_on(game, stack, pos, face, claimed);
}
