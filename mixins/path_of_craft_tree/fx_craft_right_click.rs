fn path_of_craft_tree_fx_craft_right_click(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    crate::game::pct_fx_craft_build::on_right_click(game, pos, face, canceled);
}
