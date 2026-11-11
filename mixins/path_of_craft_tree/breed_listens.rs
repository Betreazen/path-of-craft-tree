// The engine announces breeding only when a trait mod is loaded, so listen at the breeding itself.
fn path_of_craft_tree_breed_listens(game: &mut crate::game::Game<'_>, parent: u32, partner: u32) {
    let cause = game
        .love_cause
        .get(&parent)
        .or(game.love_cause.get(&partner))
        .copied();
    if let Some(player) = cause.and_then(|connection| game.killer_uuid(connection)) {
        crate::game::pct_actions::on_baby_spawn(game, player);
    }
}
