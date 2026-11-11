fn path_of_craft_tree_fx_farm_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::pct_fx_farm::on_right_click(game, pos, canceled);
}
