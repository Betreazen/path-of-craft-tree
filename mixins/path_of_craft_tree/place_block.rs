fn path_of_craft_tree_place_block(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::pct_mining::on_place(game, pos);
}
