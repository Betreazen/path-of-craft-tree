fn path_of_craft_tree_fx_farm_furnace_extract(
    game: &mut crate::game::Game<'_>,
    player: u128,
    item: &str,
    count: u32,
) {
    crate::game::pct_fx_farm_cooking::on_furnace_extract(game, player, item, count);
}
