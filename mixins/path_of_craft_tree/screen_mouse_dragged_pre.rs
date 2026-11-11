fn path_of_craft_tree_screen_mouse_dragged_pre(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, delta: [f32; 2], canceled: &mut bool) {
    crate::game::pct_ui::on_drag(game, screen, button, delta, canceled);
}
