fn path_of_craft_tree_fx_craft_send_station(game: &crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    let Some(player) = host.hub.guests.iter().find(|g| g.conn == conn).map(|g| g.uuid) else {
        return;
    };
    let Some(gs) = host.play.get_mut(&conn).and_then(|play| play.station.as_mut()) else {
        return;
    };
    crate::game::pct_fx_craft_enchant::on_station_send(game, player, &mut gs.menu, gs.kind, gs.pos);
}
