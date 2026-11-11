fn path_of_craft_tree_server_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::mod_api::ServerTickPhase,
) {
    // The engine calls this at the start and at the end of every tick; the mod works once, at the end.
    if matches!(phase, crate::mod_api::ServerTickPhase::ServerTickEnd { .. }) {
        crate::game::pct_glue::on_server_tick(game);
    }
}
