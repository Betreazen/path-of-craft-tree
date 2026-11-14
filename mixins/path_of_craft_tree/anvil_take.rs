// Every anvil take on a host ends here, deferred for the mods or not: the one place it pays.
fn path_of_craft_tree_anvil_take(
    game: &mut crate::game::Game<'_>,
    host: &crate::game::lan::LanHost,
    conn: u32,
    gs: &crate::game::lan_stations::GuestStation,
    craft: Option<&horizon_client::menu::AnvilCraft>,
) {
    if gs.kind != StationKind::Anvil || !gs.menu.combined {
        return;
    }
    if let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn) {
        crate::game::pct_anvil::on_take(game, guest.uuid, conn, -gs.menu.level_change, craft);
    }
}
