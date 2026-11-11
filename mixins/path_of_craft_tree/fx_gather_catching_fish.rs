fn path_of_craft_tree_fx_gather_catching_fish(game: &crate::game::Game<'_>, h: &mut Hook) {
    if h.nibble > 0 || h.time_until_hooked > 0 || h.time_until_lured <= 0 {
        return;
    }
    let extra = crate::game::pct_fx_gather_fish::lure_ticks(game);
    if extra > 0 {
        h.time_until_lured = (h.time_until_lured - extra).max(1);
    }
}
