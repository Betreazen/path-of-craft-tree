fn path_of_craft_tree_loot_drops(game: &mut crate::game::Game<'_>, context: &crate::game::native_hooks::NativeLootDrops<'_>, drops: &mut Vec<horizon_client::items::ItemStack>) {
    crate::game::pct_mining::on_loot(game, context, drops);
}
