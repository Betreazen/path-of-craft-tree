fn path_of_craft_tree_anvil_craft_post(game: &mut crate::game::Game<'_>, player: u128, craft: &horizon_client::menu::AnvilCraft) {
    crate::game::pct_actions::on_anvil(game, player, craft.right.as_ref());
}
