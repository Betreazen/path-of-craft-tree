fn path_of_craft_tree_fx_craft_break_speed(game: &crate::game::Game<'_>, pos: [i32; 3], speed: &mut f32) {
    crate::game::pct_fx_craft_build::on_break_speed(game, pos, speed);
}
