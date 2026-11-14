// The engine reports anvil crafts only when a trait mod is loaded, so ask for them on each click,
// and note what a take in this click would take (anvil.rs pays it where the take ends).
fn path_of_craft_tree_anvil_listens(host: &mut crate::game::lan::LanHost, conn: u32) {
    let station = host
        .play
        .get_mut(&conn)
        .and_then(|play| play.station.as_mut());
    if let Some(station) = station.filter(|station| station.kind == StationKind::Anvil) {
        station.menu.anvil_hook = true;
        crate::game::pct_anvil::before_op(conn, &station.menu);
    }
}
