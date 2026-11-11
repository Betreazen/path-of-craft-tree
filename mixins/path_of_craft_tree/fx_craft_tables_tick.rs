fn path_of_craft_tree_fx_craft_tables_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    for conn in crate::game::pct_fx_craft_tables::run(game, host) {
        game.lan_send_table_content(host, conn);
    }
}
