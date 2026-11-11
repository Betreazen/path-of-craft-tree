fn path_of_craft_tree_fx_craft_tick_furnace(game: &mut crate::game::Game<'_>, pos: (i32, i32, i32)) {
    crate::game::pct_fx_craft_items::on_tick_furnace(game, pos);
}
