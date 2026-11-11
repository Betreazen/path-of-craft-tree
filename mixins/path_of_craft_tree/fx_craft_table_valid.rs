fn path_of_craft_tree_fx_craft_table_valid(value: bool, g: &horizon_client::lan::guest::Guest, pos: [i32; 3]) -> bool {
    value || crate::game::pct_fx_craft_tables::pocket_table_valid(g.uuid, pos)
}
