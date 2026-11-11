fn path_of_craft_tree_fx_gather_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::pct_fx_gather::on_destroy(game, pos);
}
