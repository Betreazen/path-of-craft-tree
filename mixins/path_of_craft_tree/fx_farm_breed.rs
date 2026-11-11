// A second baby is simply a second run of the breeding itself, guarded against rolling again.
fn path_of_craft_tree_fx_farm_breed(game: &mut crate::game::Game<'_>, parent: u32, partner: u32) {
    let cause = game
        .love_cause
        .get(&parent)
        .or(game.love_cause.get(&partner))
        .copied();
    let Some(player) = cause.and_then(|connection| game.killer_uuid(connection)) else {
        return;
    };
    if crate::game::pct_fx_farm_husbandry::roll_twins(player) {
        game.breed(parent, partner, None);
        crate::game::pct_fx_farm_husbandry::twins_born();
    }
}
