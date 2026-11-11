fn path_of_craft_tree_fx_craft_station_valid(value: bool, g: &horizon_client::lan::guest::Guest, gs: &crate::game::lan_stations::GuestStation) -> bool {
    value || crate::game::pct_fx_craft_tables::pocket_station_valid(g.uuid, gs.kind, gs.pos)
}
