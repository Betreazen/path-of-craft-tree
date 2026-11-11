fn path_of_craft_tree_fx_farm_destroy(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::pct_fx_farm::on_destroy(game, pos, canceled);
}
