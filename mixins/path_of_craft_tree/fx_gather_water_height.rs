fn path_of_craft_tree_fx_gather_water_height(game: &crate::game::Game<'_>, x: i32, y: i32, z: i32) -> std::ops::ControlFlow<Option<(f32, bool)>> {
    if !crate::game::pct_fx_gather_fish::fishes_in_lava(game, [x, y, z]) {
        return std::ops::ControlFlow::Continue(());
    }
    let here = game.res.facts().get(game.world.block(x, y, z));
    let above = game.res.facts().get(game.world.block(x, y + 1, z));
    let height = if above.fluid == Fluid::Lava {
        1.0
    } else {
        f32::from(here.fluid_level) / 9.0
    };
    std::ops::ControlFlow::Break(Some((height, here.fluid_level == 8 && !here.fluid_falling)))
}
