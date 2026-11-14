fn path_of_craft_tree_window_screen_char(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    crate::game::pct_window::on_char(game, screen, c, canceled);
}
