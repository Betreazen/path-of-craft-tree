fn path_of_craft_tree_fx_gather_speed(game: &crate::game::Game<'_>, state: BlockStateId, speed: &mut f32) {
    let name = game.res.facts().get(state).name.clone();
    crate::game::pct_fx_gather::on_break_speed(game, &name, speed);
}
