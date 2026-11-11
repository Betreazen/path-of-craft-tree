fn path_of_craft_tree_fx_craft_prepare_smithing(game: &crate::game::Game<'_>, player: u128, result: &mut Option<horizon_client::items::ItemStack>) {
    crate::game::pct_fx_craft_items::on_prepare_smithing(game, player, result);
}
