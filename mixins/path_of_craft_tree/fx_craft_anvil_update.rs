fn path_of_craft_tree_fx_craft_anvil_update(game: &crate::game::Game<'_>, player: u128, left: &horizon_client::items::ItemStack, right: Option<&horizon_client::items::ItemStack>, item_name: Option<&str>, vanilla_output: Option<&horizon_client::items::ItemStack>, output: &mut Option<horizon_client::items::ItemStack>, xp_cost: &mut i32) {
    crate::game::pct_fx_craft_items::on_anvil_update(game, player, left, right, item_name, vanilla_output, output, xp_cost);
}
