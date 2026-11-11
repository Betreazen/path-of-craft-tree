fn path_of_craft_tree_fx_craft_open_station(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32, station: &crate::game::stations::Station) {
    if station.kind != StationKind::Enchanting {
        return;
    }
    let guest = host.hub.guests.iter_mut().find(|g| g.conn == conn);
    if let Some(guest) = guest.filter(|g| crate::game::pct_fx_craft_enchant::should_reroll(g.uuid)) {
        guest.xp_seed = game.world.random.next_int();
    }
}
