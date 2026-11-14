fn path_of_craft_tree_window_screen_key(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::pct_window::on_key(game, screen, key, modifiers, canceled);
}
