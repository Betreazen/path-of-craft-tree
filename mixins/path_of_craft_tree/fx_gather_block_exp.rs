fn path_of_craft_tree_fx_gather_block_exp(value: u32, game: &mut crate::game::Game<'_>, name: &str) -> u32 {
    crate::game::pct_fx_gather::on_block_experience(game, name, value)
}
