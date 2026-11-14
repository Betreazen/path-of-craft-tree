fn path_of_craft_tree_gains_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::mod_api::ServerTickPhase,
) {
    // Once a tick, at its end, as the mod's own server tick.
    if matches!(phase, crate::mod_api::ServerTickPhase::ServerTickEnd { .. }) {
        crate::game::pct_gains::on_server_tick(game);
    }
}
