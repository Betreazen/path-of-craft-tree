fn path_of_craft_tree_fx_roam_boat(game: &crate::game::Game<'_>, b: &mut Boat, controlled: bool) {
    let forward = b.input[2];
    let yaw = b.yaw;
    crate::game::pct_fx_roam_world::boat_thrust(game, &mut b.velocity, yaw, forward, controlled);
}
