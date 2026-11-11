fn path_of_craft_tree_fx_craft_station_combined(host: &crate::game::lan::LanHost, conn: u32, gs: &crate::game::lan_stations::GuestStation) {
    if gs.kind != StationKind::Anvil {
        return;
    }
    if let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn) {
        crate::game::pct_fx_craft::note_anvil_levels(guest.uuid, -gs.menu.level_change);
    }
}
