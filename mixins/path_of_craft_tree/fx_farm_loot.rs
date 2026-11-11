fn path_of_craft_tree_fx_farm_loot(
    context: &crate::game::native_hooks::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::pct_fx_farm::on_loot(context, drops);
}
