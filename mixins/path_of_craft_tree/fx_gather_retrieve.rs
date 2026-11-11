fn path_of_craft_tree_fx_gather_retrieve(game: &mut crate::game::Game<'_>) {
    let hook_block = game.hook.as_ref().map(|h| {
        [
            h.position.x.floor() as i32,
            h.position.y.floor() as i32,
            h.position.z.floor() as i32,
        ]
    });
    crate::game::pct_fx_gather_fish::on_retrieve(game, hook_block);
    let hooked = game.hook.as_ref().and_then(|h| match h.state {
        State::Hooked(id) => Some((id, h.position)),
        _ => None,
    });
    let (Some((id, at)), Some(damage), Some(owner)) = (
        hooked,
        crate::game::pct_fx_gather_fish::harpoon(game),
        game.player.as_ref().map(|p| p.position),
    ) else {
        return;
    };
    let Some(mob) = game.mobs.iter_mut().find(|m| m.id == id && m.alive()) else {
        return;
    };
    mob.velocity = pull_hooked(mob.velocity, at, owner);
    mob.needs_sync = true;
    if damage > 0.0 {
        mob.health = (mob.health - damage).max(0.5);
    }
}
