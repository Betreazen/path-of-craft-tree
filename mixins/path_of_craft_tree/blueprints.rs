// The Blueprint node's saved prints, free of game types so tests can run. A print is kept per
// player in the world's mod folder (blueprints/<player>.json), by block names and properties:
// block state numbers depend on the game build and the loaded mods, names do not.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The file layout; a file in another one is not read.
pub const FORMAT: u32 = 1;
/// The largest box side a file may describe. The tree's Blueprint stops well below it; the bound
/// only keeps a damaged or hand-edited file from asking for an enormous paste.
pub const MAX_SIDE: i32 = 32;

/// One block of a print, at its offset from the box's lowest corner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub at: [i32; 3],
    pub block: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

/// A saved print: the box's extent along each axis (0 for one block) and its non-air blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub format: u32,
    pub size: [i32; 3],
    pub cells: Vec<Cell>,
}

impl Saved {
    pub fn new(size: [i32; 3], cells: Vec<Cell>) -> Self {
        Saved {
            format: FORMAT,
            size,
            cells,
        }
    }
}

/// The file name of `player`'s print inside the blueprints folder.
pub fn file_name(player: u128) -> String {
    format!("{player:032x}.json")
}

pub fn encode(saved: &Saved) -> Vec<u8> {
    serde_json::to_vec(saved).unwrap_or_default()
}

/// A print read back, or None when the file is not one this version wrote or describes blocks
/// outside its own box.
pub fn decode(bytes: &[u8]) -> Option<Saved> {
    let saved: Saved = serde_json::from_slice(bytes).ok()?;
    let size_ok = saved.size.iter().all(|side| (0..MAX_SIDE).contains(side));
    if saved.format != FORMAT || !size_ok {
        return None;
    }
    let inside = |cell: &Cell| (0..3).all(|axis| (0..=saved.size[axis]).contains(&cell.at[axis]));
    let volume: usize = saved.size.iter().map(|side| *side as usize + 1).product();
    let cells_ok = saved.cells.len() <= volume
        && saved
            .cells
            .iter()
            .all(|cell| inside(cell) && !cell.block.is_empty());
    cells_ok.then_some(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stairs() -> Cell {
        Cell {
            at: [1, 0, 2],
            block: "minecraft:oak_stairs".to_owned(),
            properties: BTreeMap::from([
                ("facing".to_owned(), "east".to_owned()),
                ("half".to_owned(), "bottom".to_owned()),
            ]),
        }
    }

    fn print() -> Saved {
        let stone = Cell {
            at: [0, 0, 0],
            block: "minecraft:stone".to_owned(),
            properties: BTreeMap::new(),
        };
        Saved::new([2, 1, 2], vec![stone, stairs()])
    }

    #[test]
    fn a_print_reads_back_as_it_was_saved() {
        let saved = print();
        assert_eq!(decode(&encode(&saved)), Some(saved));
    }

    #[test]
    fn a_block_without_properties_saves_without_the_field() {
        let text = String::from_utf8(encode(&print())).unwrap();
        assert_eq!(text.matches("properties").count(), 1);
    }

    #[test]
    fn a_file_of_another_format_is_not_read() {
        let mut saved = print();
        saved.format = FORMAT + 1;
        assert_eq!(decode(&encode(&saved)), None);
    }

    #[test]
    fn a_block_outside_the_box_or_a_huge_box_is_not_read() {
        let mut outside = print();
        outside.cells[1].at = [3, 0, 0];
        assert_eq!(decode(&encode(&outside)), None);
        let mut negative = print();
        negative.cells[0].at = [0, -1, 0];
        assert_eq!(decode(&encode(&negative)), None);
        let huge = Saved::new([MAX_SIDE, 0, 0], Vec::new());
        assert_eq!(decode(&encode(&huge)), None);
    }

    #[test]
    fn more_cells_than_the_box_holds_are_not_read() {
        let one = Cell {
            at: [0, 0, 0],
            block: "minecraft:stone".to_owned(),
            properties: BTreeMap::new(),
        };
        let crowded = Saved::new([0, 0, 0], vec![one.clone(), one]);
        assert_eq!(decode(&encode(&crowded)), None);
    }

    #[test]
    fn garbage_is_not_read() {
        assert_eq!(decode(b"not json"), None);
        assert_eq!(decode(b"{}"), None);
    }

    #[test]
    fn the_file_is_named_after_the_player() {
        assert_eq!(file_name(0xab), format!("{:0>32}.json", "ab"));
    }
}
