fn path_of_craft_tree_furnace_extract(game: &mut crate::game::Game<'_>, player: u128, item: &str, count: u32) {
    crate::game::pct_actions::on_furnace_extract(game, player, item, count);
}
