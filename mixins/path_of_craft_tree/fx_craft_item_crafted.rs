fn path_of_craft_tree_fx_craft_item_crafted(game: &mut crate::game::Game<'_>, player: u128, recipe: &str, stack: &horizon_client::items::ItemStack) {
    crate::game::pct_fx_craft_items::on_item_crafted(game, player, recipe, stack);
}
