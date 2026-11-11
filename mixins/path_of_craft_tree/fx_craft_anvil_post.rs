fn path_of_craft_tree_fx_craft_anvil_post(game: &mut crate::game::Game<'_>, player: u128, craft: &horizon_client::menu::AnvilCraft) {
    crate::game::pct_fx_craft_enchant::on_anvil_post(game, player, craft);
}
