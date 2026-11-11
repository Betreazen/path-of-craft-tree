fn path_of_craft_tree_fx_gather_fishing_loot(loot: &horizon_world::loot::LootData, sequences: &mut horizon_client::random_sequences::RandomSequences, level_random: &mut horizon_core::random::LegacyRandom, world_seed: i64, luck: f32, biome: Option<&str>, open_water: bool) -> std::ops::ControlFlow<Vec<horizon_world::loot::LootStack>> {
    crate::game::pct_fx_gather_fish::roll_catch(loot, sequences, level_random, world_seed, luck, biome, open_water)
}
