fn path_of_craft_tree_fx_gather_loot(game: &mut crate::game::Game<'_>, context: &crate::game::native_hooks::NativeLootDrops<'_>, drops: &mut Vec<horizon_client::items::ItemStack>) {
    crate::game::pct_fx_gather::on_final_loot(game, context, drops);
}
