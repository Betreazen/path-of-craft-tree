fn path_of_craft_tree_brewing_complete(game: &crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::pct_actions::on_brewing_complete(game, pos);
}
