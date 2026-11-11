fn path_of_craft_tree_fx_gather_hurt_enemy(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, entity: u32) {
    crate::game::pct_fx_gather_dig::on_hurt_enemy(game, stack, entity);
}
