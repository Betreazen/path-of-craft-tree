fn path_of_craft_tree_fx_craft_anvil_used(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> std::ops::ControlFlow<bool> {
    const ANVIL_USED_EVENT: i32 = 1030;
    if crate::game::pct_fx_craft_items::anvil_keeps(pos) {
        game.level_event_at(ANVIL_USED_EVENT, pos, 0);
        return std::ops::ControlFlow::Break(false);
    }
    std::ops::ControlFlow::Continue(())
}
