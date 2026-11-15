fn path_of_craft_tree_hud_bar(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::pct_hud_bar::draw(game, batch);
}
