// Which blocks players placed, by region, so breaking them earns no experience.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

const REGION_BITS: i32 = 9;
const REGION_SIDE: i32 = 1 << REGION_BITS;
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

    /// The regions within `margin` blocks of `position` across x and z: one to four of them.
    pub fn near(dimension: &str, position: Position, margin: i32) -> Vec<RegionKey> {
        let margin = margin.clamp(0, REGION_SIDE / 2);
        let span = |at: i32| {
            let low = at.saturating_sub(margin) >> REGION_BITS;
            let high = at.saturating_add(margin) >> REGION_BITS;
            low..=high
        };
        let mut keys = Vec::with_capacity(4);
        for x in span(position.0) {
            for z in span(position.2) {
                keys.push(RegionKey {
                    dimension: dimension.to_owned(),
                    x,
                    z,
                });
            }
        }
        keys
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

/// The regions loaded so far. `read` supplies a region's saved bytes the first time it is used,
/// unless a read in the background (`claim`, then `merge`) brought the region in before that.
#[derive(Debug, Default)]
pub struct Placed {
    regions: HashMap<RegionKey, HashSet<Position>>,
    dirty: BTreeSet<RegionKey>,
    /// Regions being read in the background.
    reading: HashSet<RegionKey>,
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

    /// Claims `key` for a read in the background: true when it is neither loaded nor being read.
    pub fn claim(&mut self, key: &RegionKey) -> bool {
        !self.regions.contains_key(key) && self.reading.insert(key.clone())
    }

    /// Takes in a region read in the background; `None` when the read failed, which leaves the
    /// region to the synchronous read. A region loaded meanwhile, because a block in it was
    /// needed first, is kept: its marks are at least as new as the file was. True when merged.
    pub fn merge(&mut self, key: RegionKey, bytes: Option<Vec<u8>>) -> bool {
        self.reading.remove(&key);
        if self.regions.contains_key(&key) {
            return false;
        }
        let Some(bytes) = bytes else {
            return false;
        };
        self.regions.insert(key, decode(&bytes).unwrap_or_default());
        true
    }

    pub fn is_loaded(&self, key: &RegionKey) -> bool {
        self.regions.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.regions.values().map(HashSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A region file read off the server thread.
#[derive(Debug)]
pub struct Fetched {
    pub dir: PathBuf,
    pub key: RegionKey,
    /// The file's bytes, empty when there is no file yet; `None` when it could not be read.
    pub bytes: Option<Vec<u8>>,
    pub took: Duration,
}

/// Reads region files on a thread of its own, so the server thread finds them loaded.
pub struct Prefetcher {
    asks: Sender<(PathBuf, RegionKey)>,
    done: Receiver<Fetched>,
}

impl Prefetcher {
    /// Starts the reading thread; `None` when the system refuses one.
    pub fn start() -> Option<Prefetcher> {
        let (asks, asked) = channel::<(PathBuf, RegionKey)>();
        let (finished, done) = channel();
        std::thread::Builder::new()
            .name("pct-placed-prefetch".to_owned())
            .spawn(move || {
                for (dir, key) in asked {
                    let started = Instant::now();
                    let bytes = match std::fs::read(dir.join(key.file_name())) {
                        Ok(bytes) => Some(bytes),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            Some(Vec::new())
                        }
                        Err(_) => None,
                    };
                    let took = started.elapsed();
                    let fetched = Fetched {
                        dir,
                        key,
                        bytes,
                        took,
                    };
                    if finished.send(fetched).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Prefetcher { asks, done })
    }

    /// Asks for `key`'s file in `dir`; false when the reading thread is gone.
    pub fn ask(&self, dir: PathBuf, key: RegionKey) -> bool {
        self.asks.send((dir, key)).is_ok()
    }

    /// The reads finished since the last call, without waiting.
    pub fn finished(&self) -> Vec<Fetched> {
        self.done.try_iter().collect()
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

    fn saved_bytes(positions: &[Position]) -> Vec<u8> {
        encode(&positions.iter().copied().collect())
    }

    #[test]
    fn regions_near_a_position_reach_across_borders_only_when_close() {
        let inside = RegionKey::near(OVERWORLD, (100, 64, 100), 32);
        assert_eq!(inside, vec![RegionKey::of(OVERWORLD, (100, 64, 100))]);
        let corner = RegionKey::near(OVERWORLD, (5, 64, -3), 32);
        assert_eq!(corner.len(), 4);
        for at in [(-27, 0, -35), (37, 0, -35), (-27, 0, 29), (37, 0, 29)] {
            assert!(corner.contains(&RegionKey::of(OVERWORLD, at)), "{at:?}");
        }
    }

    #[test]
    fn a_prefetched_region_answers_without_reading_again() {
        let mut placed = Placed::default();
        let key = RegionKey::of(OVERWORLD, (3, 64, 3));
        assert!(placed.claim(&key));
        assert!(!placed.claim(&key), "asked for twice");
        assert!(placed.merge(key.clone(), Some(saved_bytes(&[(3, 64, 3)]))));
        assert!(!placed.claim(&key), "already loaded");
        let no_read = |_: &RegionKey| -> Option<Vec<u8>> { panic!("read again") };
        assert!(placed.contains(OVERWORLD, (3, 64, 3), no_read));
        assert!(
            placed.take_dirty().is_empty(),
            "a merged region is not a change"
        );
    }

    #[test]
    fn a_region_read_meanwhile_keeps_its_newer_marks() {
        let mut placed = Placed::default();
        let key = RegionKey::of(OVERWORLD, (3, 64, 3));
        let file = saved_bytes(&[(3, 64, 3)]);
        assert!(placed.claim(&key));
        // The block is broken before the background read comes back: read synchronously.
        placed.remove(OVERWORLD, (3, 64, 3), |_| Some(file.clone()));
        placed.add(OVERWORLD, (4, 64, 4), nothing);
        assert!(!placed.merge(key.clone(), Some(file)));
        assert!(!placed.contains(OVERWORLD, (3, 64, 3), nothing));
        assert!(placed.contains(OVERWORLD, (4, 64, 4), nothing));
        assert_eq!(placed.take_dirty().len(), 1);
        assert!(!placed.claim(&key));
    }

    #[test]
    fn a_saved_region_ignores_a_stale_background_read() {
        // Read synchronously, changed and saved while the background read of the old file ran.
        let mut placed = Placed::default();
        let key = RegionKey::of(OVERWORLD, (3, 64, 3));
        let old = saved_bytes(&[(3, 64, 3)]);
        assert!(placed.claim(&key));
        placed.remove(OVERWORLD, (3, 64, 3), |_| Some(old.clone()));
        assert_eq!(placed.take_dirty().len(), 1);
        assert!(!placed.merge(key, Some(old)));
        assert!(!placed.contains(OVERWORLD, (3, 64, 3), nothing));
        assert!(placed.take_dirty().is_empty());
    }

    #[test]
    fn a_failed_background_read_leaves_the_region_to_the_synchronous_read() {
        let mut placed = Placed::default();
        let key = RegionKey::of(OVERWORLD, (3, 64, 3));
        assert!(placed.claim(&key));
        assert!(!placed.merge(key.clone(), None));
        assert!(!placed.is_loaded(&key));
        let file = saved_bytes(&[(3, 64, 3)]);
        assert!(placed.contains(OVERWORLD, (3, 64, 3), |_| Some(file.clone())));
    }

    #[test]
    fn a_missing_file_is_an_empty_region_and_a_corrupt_one_counts_as_empty() {
        let mut placed = Placed::default();
        let (empty, corrupt) = (
            RegionKey::of(OVERWORLD, (0, 0, 0)),
            RegionKey::of(OVERWORLD, (600, 0, 0)),
        );
        assert!(placed.claim(&empty) && placed.claim(&corrupt));
        assert!(placed.merge(empty.clone(), Some(Vec::new())));
        assert!(placed.merge(corrupt.clone(), Some(vec![1, 2, 3])));
        assert!(placed.is_loaded(&empty) && placed.is_loaded(&corrupt));
        assert!(!placed.contains(OVERWORLD, (600, 0, 0), nothing));
    }

    #[test]
    fn moved_marks_land_in_a_prefetched_region() {
        // A piston pushes a placed block over a border into a region read in the background.
        let mut placed = Placed::default();
        let far = RegionKey::of(OVERWORLD, (512, 0, 0));
        assert!(placed.claim(&far));
        assert!(placed.merge(far, Some(saved_bytes(&[(600, 70, 0)]))));
        placed.add(OVERWORLD, (511, 70, 0), nothing);
        placed.remove(OVERWORLD, (511, 70, 0), nothing);
        placed.add(OVERWORLD, (512, 70, 0), nothing);
        let saved = placed.take_dirty();
        let region = saved
            .iter()
            .find(|(key, _)| key.x == 1)
            .and_then(|(_, bytes)| decode(bytes))
            .unwrap();
        assert_eq!(region, HashSet::from([(512, 70, 0), (600, 70, 0)]));
    }

    #[test]
    fn the_prefetcher_reads_files_and_reports_missing_ones_as_empty() {
        let dir = std::env::temp_dir().join(format!("pct-placed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let there = RegionKey::of(OVERWORLD, (1, 0, 1));
        let missing = RegionKey::of(OVERWORLD, (5000, 0, 1));
        std::fs::write(dir.join(there.file_name()), saved_bytes(&[(1, 2, 3)])).unwrap();
        let prefetcher = Prefetcher::start().unwrap();
        assert!(prefetcher.ask(dir.clone(), there.clone()));
        assert!(prefetcher.ask(dir.clone(), missing.clone()));
        let mut fetched = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while fetched.len() < 2 && Instant::now() < deadline {
            fetched.extend(prefetcher.finished());
            std::thread::yield_now();
        }
        std::fs::remove_dir_all(&dir).unwrap();
        let bytes = |key: &RegionKey| {
            fetched
                .iter()
                .find(|f| &f.key == key && f.dir == dir)
                .and_then(|f| f.bytes.clone())
        };
        assert_eq!(fetched.len(), 2);
        assert_eq!(bytes(&there), Some(saved_bytes(&[(1, 2, 3)])));
        assert_eq!(bytes(&missing), Some(Vec::new()));
    }
}
