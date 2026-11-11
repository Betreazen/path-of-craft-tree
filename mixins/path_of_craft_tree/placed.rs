// Which blocks players placed, by region, so breaking them earns no experience.
use std::collections::{BTreeSet, HashMap, HashSet};

const REGION_BITS: i32 = 9;
const POSITION_BYTES: usize = 12;

pub type Position = (i32, i32, i32);

/// One 512×512 column of a dimension.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionKey {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
}

impl RegionKey {
    pub fn of(dimension: &str, position: Position) -> RegionKey {
        RegionKey {
            dimension: dimension.to_owned(),
            x: position.0 >> REGION_BITS,
            z: position.2 >> REGION_BITS,
        }
    }

    /// The file name, unique per dimension and region.
    pub fn file_name(&self) -> String {
        let dimension = self.dimension.replace(':', "_");
        format!("{dimension}.{}.{}.bin", self.x, self.z)
    }
}

pub fn encode(positions: &HashSet<Position>) -> Vec<u8> {
    positions
        .iter()
        .flat_map(|(x, y, z)| [x.to_le_bytes(), y.to_le_bytes(), z.to_le_bytes()].concat())
        .collect()
}

pub fn decode(bytes: &[u8]) -> Option<HashSet<Position>> {
    if !bytes.len().is_multiple_of(POSITION_BYTES) {
        return None;
    }
    let number = |at: &[u8]| i32::from_le_bytes([at[0], at[1], at[2], at[3]]);
    Some(
        bytes
            .as_chunks::<POSITION_BYTES>()
            .0
            .iter()
            .map(|entry| {
                (
                    number(&entry[0..4]),
                    number(&entry[4..8]),
                    number(&entry[8..12]),
                )
            })
            .collect(),
    )
}

/// The regions loaded so far. `read` supplies a region's saved bytes the first time it is used.
#[derive(Debug, Default)]
pub struct Placed {
    regions: HashMap<RegionKey, HashSet<Position>>,
    dirty: BTreeSet<RegionKey>,
}

impl Placed {
    fn region(
        &mut self,
        key: RegionKey,
        read: impl FnOnce(&RegionKey) -> Option<Vec<u8>>,
    ) -> &mut HashSet<Position> {
        self.regions.entry(key).or_insert_with_key(|key| {
            read(key)
                .and_then(|bytes| decode(&bytes))
                .unwrap_or_default()
        })
    }

    pub fn add(
        &mut self,
        dimension: &str,
        position: Position,
        read: impl FnOnce(&RegionKey) -> Option<Vec<u8>>,
    ) {
        let key = RegionKey::of(dimension, position);
        if self.region(key.clone(), read).insert(position) {
            self.dirty.insert(key);
        }
    }

    pub fn contains(
        &mut self,
        dimension: &str,
        position: Position,
        read: impl FnOnce(&RegionKey) -> Option<Vec<u8>>,
    ) -> bool {
        let key = RegionKey::of(dimension, position);
        self.region(key, read).contains(&position)
    }

    pub fn remove(
        &mut self,
        dimension: &str,
        position: Position,
        read: impl FnOnce(&RegionKey) -> Option<Vec<u8>>,
    ) {
        let key = RegionKey::of(dimension, position);
        if self.region(key.clone(), read).remove(&position) {
            self.dirty.insert(key);
        }
    }

    /// The changed regions and their bytes; they count as saved afterwards.
    pub fn take_dirty(&mut self) -> Vec<(RegionKey, Vec<u8>)> {
        let keys = std::mem::take(&mut self.dirty);
        keys.into_iter()
            .filter_map(|key| {
                let bytes = encode(self.regions.get(&key)?);
                Some((key, bytes))
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.regions.values().map(HashSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERWORLD: &str = "minecraft:overworld";

    fn nothing(_: &RegionKey) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn a_placed_block_is_remembered_until_removed() {
        let mut placed = Placed::default();
        placed.add(OVERWORLD, (1, 64, 1), nothing);
        assert!(placed.contains(OVERWORLD, (1, 64, 1), nothing));
        assert!(!placed.contains(OVERWORLD, (2, 64, 1), nothing));
        placed.remove(OVERWORLD, (1, 64, 1), nothing);
        assert!(!placed.contains(OVERWORLD, (1, 64, 1), nothing));
    }

    #[test]
    fn dimensions_are_separate() {
        let mut placed = Placed::default();
        placed.add(OVERWORLD, (1, 64, 1), nothing);
        assert!(!placed.contains("minecraft:the_nether", (1, 64, 1), nothing));
    }

    #[test]
    fn negative_coordinates_land_in_their_own_region() {
        assert_ne!(
            RegionKey::of(OVERWORLD, (-1, 0, 0)),
            RegionKey::of(OVERWORLD, (0, 0, 0))
        );
    }

    #[test]
    fn saved_regions_round_trip_through_their_bytes() {
        let mut placed = Placed::default();
        placed.add(OVERWORLD, (-5, 12, 700), nothing);
        placed.add(OVERWORLD, (-6, 12, 700), nothing);
        let saved = placed.take_dirty();
        assert_eq!(saved.len(), 1);
        let (key, bytes) = saved[0].clone();
        let mut reloaded = Placed::default();
        let read = |asked: &RegionKey| (asked == &key).then(|| bytes.clone());
        assert!(reloaded.contains(OVERWORLD, (-5, 12, 700), read));
    }

    #[test]
    fn saving_clears_the_dirty_list_until_the_next_change() {
        let mut placed = Placed::default();
        placed.add(OVERWORLD, (0, 0, 0), nothing);
        assert_eq!(placed.take_dirty().len(), 1);
        assert!(placed.take_dirty().is_empty());
        placed.remove(OVERWORLD, (0, 0, 0), nothing);
        assert_eq!(placed.take_dirty().len(), 1);
    }

    #[test]
    fn a_corrupt_region_counts_as_empty() {
        let mut placed = Placed::default();
        assert!(!placed.contains(OVERWORLD, (0, 0, 0), |_| Some(vec![1, 2, 3])));
    }
}
