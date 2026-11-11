fn path_of_craft_tree_enchant_item(game: &mut crate::game::Game<'_>, player: u128, levels: i32) {
    crate::game::pct_actions::on_enchant(game, player, levels);
}
