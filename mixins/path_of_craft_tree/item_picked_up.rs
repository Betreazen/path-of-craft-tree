fn path_of_craft_tree_item_picked_up(game: &mut crate::game::Game<'_>, player: u128, item: &str, count: u32) {
    crate::game::pct_actions::on_item_picked_up(game, player, item, count);
}
