fn path_of_craft_tree_fx_roam_far_kill(game: &mut crate::game::Game<'_>, context: &crate::game::native_hooks::NativeLootDrops<'_>) {
    crate::game::pct_fx_roam_combat::on_kill_loot(game, context);
}
