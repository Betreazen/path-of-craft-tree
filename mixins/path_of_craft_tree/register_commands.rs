fn path_of_craft_tree_register_commands(commands: &mut Vec<crate::mod_api::NativeCommand>) {
    crate::game::pct_machines::on_register_commands(commands);
    crate::game::pct_selftest::on_register_commands(commands);
}
