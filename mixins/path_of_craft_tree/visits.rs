// The chunks a player has visited, for the explorer's one-time chunk experience. Kept compact in
// memory (a chunk is one u64 per dimension) and saved by appending only the chunks new since the
// last save, so a far explorer with hundreds of thousands of chunks costs a few milliseconds a save
// and at login instead of hundreds. Free of game types so the tests run without the engine.
//
// File: the magic `PCV1`, then blocks. A block is a u32 length (little endian) of its payload, then
// the payload: a u8 dimension name length, the name, and its chunks as u64 (little endian). A block
// cut short by a crash mid-write is dropped when read, with only that save's new chunks lost.
use std::collections::{HashMap, HashSet};

pub const MAGIC: &[u8; 4] = b"PCV1";

/// The most chunks one block holds, so a block's length always fits its u32.
const MAX_BLOCK_CHUNKS: usize = 1 << 24;

/// A chunk position as one number: x in the high half, z in the low half.
pub fn pack(x: i32, z: i32) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(z as u32)
}

pub fn unpack(packed: u64) -> (i32, i32) {
    ((packed >> 32) as u32 as i32, packed as u32 as i32)
}

/// One player's visited chunks, and which of them are not saved yet.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Visits {
    by_dimension: HashMap<String, HashSet<u64>>,
    unsaved: HashMap<String, Vec<u64>>,
}

impl Visits {
    /// Records chunk `x`, `z` of `dimension`; true when it is new.
    pub fn insert(&mut self, dimension: &str, x: i32, z: i32) -> bool {
        let chunk = pack(x, z);
        let known = match self.by_dimension.get_mut(dimension) {
            Some(set) => !set.insert(chunk),
            None => {
                self.by_dimension
                    .insert(dimension.to_owned(), HashSet::from([chunk]));
                false
            }
        };
        if known {
            return false;
        }
        self.unsaved
            .entry(dimension.to_owned())
            .or_default()
            .push(chunk);
        true
    }

    pub fn contains(&self, dimension: &str, x: i32, z: i32) -> bool {
        self.by_dimension
            .get(dimension)
            .is_some_and(|set| set.contains(&pack(x, z)))
    }

    pub fn len(&self) -> usize {
        self.by_dimension.values().map(HashSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn has_unsaved(&self) -> bool {
        self.unsaved.values().any(|chunks| !chunks.is_empty())
    }

    /// The blocks to append for the chunks not saved yet; empty when there are none. `with_magic`
    /// starts a new file.
    pub fn unsaved_bytes(&self, with_magic: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if with_magic {
            out.extend_from_slice(MAGIC);
        }
        let mut dimensions: Vec<&String> = self.unsaved.keys().collect();
        dimensions.sort();
        for dimension in dimensions {
            for chunks in self.unsaved[dimension].chunks(MAX_BLOCK_CHUNKS) {
                push_block(&mut out, dimension, chunks);
            }
        }
        out
    }

    /// Everything as a whole new file: for moving an old save over to this format.
    pub fn all_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let mut dimensions: Vec<&String> = self.by_dimension.keys().collect();
        dimensions.sort();
        for dimension in dimensions {
            let mut chunks: Vec<u64> = self.by_dimension[dimension].iter().copied().collect();
            chunks.sort_unstable();
            for part in chunks.chunks(MAX_BLOCK_CHUNKS) {
                push_block(&mut out, dimension, part);
            }
        }
        out
    }

    /// After a successful write: everything known is saved.
    pub fn mark_saved(&mut self) {
        self.unsaved.clear();
    }

    /// Reads a file of this format. A file without the magic reads as nothing (`Err`), a block
    /// cut short at the end is dropped.
    pub fn decode(bytes: &[u8]) -> Result<Visits, String> {
        let Some(mut rest) = bytes.strip_prefix(MAGIC.as_slice()) else {
            return Err("not a visited-chunks file".to_owned());
        };
        let mut visits = Visits::default();
        while rest.len() >= 4 {
            let len = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
            let Some(payload) = rest.get(4..4 + len) else {
                break;
            };
            rest = &rest[4 + len..];
            let Some((&name_len, payload)) = payload.split_first() else {
                continue;
            };
            let name_len = usize::from(name_len);
            if payload.len() < name_len || (payload.len() - name_len) % 8 != 0 {
                continue;
            }
            let (name, chunks) = payload.split_at(name_len);
            let Ok(name) = std::str::from_utf8(name) else {
                continue;
            };
            let set = visits.by_dimension.entry(name.to_owned()).or_default();
            let (words, _) = chunks.as_chunks::<8>();
            set.reserve(words.len());
            for word in words {
                set.insert(u64::from_le_bytes(*word));
            }
        }
        Ok(visits)
    }

    /// Chunks from the old JSON save, all unsaved, so the first save writes them in this format.
    pub fn from_legacy(chunks: impl IntoIterator<Item = (String, i32, i32)>) -> Visits {
        let mut visits = Visits::default();
        for (dimension, x, z) in chunks {
            visits.insert(&dimension, x, z);
        }
        visits
    }
}

fn push_block(out: &mut Vec<u8>, dimension: &str, chunks: &[u64]) {
    let name = &dimension.as_bytes()[..dimension.len().min(usize::from(u8::MAX))];
    let len = 1 + name.len() + chunks.len() * 8;
    out.extend_from_slice(&(len as u32).to_le_bytes());
    out.push(name.len() as u8);
    out.extend_from_slice(name);
    for chunk in chunks {
        out.extend_from_slice(&chunk.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_keeps_negative_and_far_coordinates() {
        for (x, z) in [
            (0, 0),
            (-1, 1),
            (i32::MIN, i32::MAX),
            (1_875_000, -1_875_000),
            (-3, -7),
        ] {
            assert_eq!(unpack(pack(x, z)), (x, z));
        }
        assert_ne!(pack(1, 2), pack(2, 1));
    }

    #[test]
    fn only_new_chunks_are_unsaved_and_saving_clears_them() {
        let mut visits = Visits::default();
        assert!(visits.insert("minecraft:overworld", 1, 2));
        assert!(!visits.insert("minecraft:overworld", 1, 2));
        assert!(visits.insert("minecraft:the_nether", 1, 2));
        assert_eq!(visits.len(), 2);
        assert!(visits.has_unsaved());
        visits.mark_saved();
        assert!(!visits.has_unsaved());
        assert!(visits.unsaved_bytes(false).is_empty());
        assert!(!visits.insert("minecraft:overworld", 1, 2));
        assert!(!visits.has_unsaved());
    }

    #[test]
    fn appended_blocks_read_back_as_the_whole_set() {
        let mut visits = Visits::default();
        for i in 0..1000 {
            visits.insert("minecraft:overworld", i, -i);
        }
        let mut file = visits.unsaved_bytes(true);
        visits.mark_saved();
        for i in 0..50 {
            visits.insert("minecraft:the_end", -i, i);
            visits.insert("minecraft:overworld", i, -i);
        }
        file.extend(visits.unsaved_bytes(false));
        visits.mark_saved();
        let read = Visits::decode(&file).unwrap();
        assert_eq!(read.len(), 1050);
        assert_eq!(read, visits);
        assert_eq!(file.len(), 4 + (4 + 1 + 19 + 8000) + (4 + 1 + 17 + 400));
    }

    #[test]
    fn a_block_cut_short_is_dropped_and_the_rest_kept() {
        let mut visits = Visits::default();
        visits.insert("minecraft:overworld", 5, 5);
        let mut file = visits.unsaved_bytes(true);
        visits.mark_saved();
        visits.insert("minecraft:overworld", 6, 6);
        let second = visits.unsaved_bytes(false);
        file.extend_from_slice(&second[..second.len() - 3]);
        let read = Visits::decode(&file).unwrap();
        assert!(read.contains("minecraft:overworld", 5, 5));
        assert!(!read.contains("minecraft:overworld", 6, 6));
        assert!(Visits::decode(b"[[\"minecraft:overworld\",1,2]]").is_err());
    }

    #[test]
    fn an_old_save_moves_over_whole() {
        let old = vec![
            ("minecraft:overworld".to_owned(), 1, 2),
            ("minecraft:overworld".to_owned(), -4, 9),
            ("minecraft:the_nether".to_owned(), 0, 0),
        ];
        let visits = Visits::from_legacy(old);
        assert_eq!(visits.len(), 3);
        assert!(visits.has_unsaved());
        let read = Visits::decode(&visits.unsaved_bytes(true)).unwrap();
        assert_eq!(read.len(), 3);
        assert_eq!(Visits::decode(&visits.all_bytes()).unwrap().len(), 3);
    }
}
