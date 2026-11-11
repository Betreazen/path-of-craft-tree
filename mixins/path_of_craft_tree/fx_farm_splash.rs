fn path_of_craft_tree_fx_farm_splash(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, at: Vec3) {
    crate::game::pct_fx_farm_alchemy::on_splash(game, stack, [at.x, at.y, at.z]);
}
