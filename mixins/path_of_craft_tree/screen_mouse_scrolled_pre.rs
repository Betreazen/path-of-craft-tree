fn path_of_craft_tree_screen_mouse_scrolled_pre(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::pct_ui::on_scroll(game, screen, scroll, canceled);
}
