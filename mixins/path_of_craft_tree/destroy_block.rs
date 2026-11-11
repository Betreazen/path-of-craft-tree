fn path_of_craft_tree_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::pct_mining::on_destroy(game, pos);
}
