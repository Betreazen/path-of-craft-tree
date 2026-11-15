// The leaderboard file for the server's site, free of game types: one row for each player who
// agreed with `/pct leaderboard on`. The format is in docs/leaderboard.md. leaderboard.rs keeps it
// in memory and writes it to the world folder.
use crate::game::pct_rules::{Profile, Tree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The file's name in `<world>/path_of_craft_tree/`.
pub const FILE_NAME: &str = "leaderboard.json";
/// The shape of [`Board`]; raised only when readers must change. New fields come without it.
pub const BOARD_VERSION: u32 = 1;

/// One player who agreed to be shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// 32 hex digits, no dashes.
    pub uuid: String,
    pub name: String,
    /// The level of every branch of the tree, by branch id.
    pub levels: BTreeMap<String, u32>,
    /// The branch levels added up.
    pub total: u32,
    /// The general scale's level.
    pub general: u32,
    /// When this row was last worked out, in Unix seconds.
    pub updated: u64,
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub v: u32,
    /// When the file was last written, in Unix seconds.
    pub updated: u64,
    /// The highest level of a branch.
    pub max_level: u32,
    /// Branch ids with the names the game shows.
    pub branches: BTreeMap<String, String>,
    /// Highest total first; equal totals by name.
    pub players: Vec<Row>,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            v: BOARD_VERSION,
            updated: 0,
            max_level: 0,
            branches: BTreeMap::new(),
            players: Vec::new(),
        }
    }
}

/// A player's row as `tree` reads `profile`.
pub fn row(tree: &Tree, profile: &Profile, uuid: u128, name: &str, now: u64) -> Row {
    let levels: BTreeMap<String, u32> = tree
        .branches
        .keys()
        .map(|branch| (branch.clone(), tree.branch_level(profile, branch)))
        .collect();
    Row {
        uuid: format!("{uuid:032x}"),
        name: name.to_owned(),
        total: levels.values().sum(),
        levels,
        general: tree.general_level(profile),
        updated: now,
    }
}

impl Board {
    pub fn parse(bytes: &[u8]) -> Result<Board, String> {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }

    pub fn has(&self, uuid: u128) -> bool {
        let id = format!("{uuid:032x}");
        self.players.iter().any(|row| row.uuid == id)
    }

    /// Puts in or replaces the player's row.
    pub fn upsert(&mut self, row: Row) {
        match self.players.iter_mut().find(|r| r.uuid == row.uuid) {
            Some(old) => *old = row,
            None => self.players.push(row),
        }
    }

    /// Takes the player out; true when they were in.
    pub fn remove(&mut self, uuid: u128) -> bool {
        let id = format!("{uuid:032x}");
        let before = self.players.len();
        self.players.retain(|row| row.uuid != id);
        self.players.len() != before
    }

    /// The file's bytes: the tree's branches and limits refreshed, rows in leaderboard order.
    pub fn encode(&self, tree: &Tree, now: u64) -> Result<Vec<u8>, String> {
        let mut board = self.clone();
        board.v = BOARD_VERSION;
        board.updated = now;
        board.max_level = tree.curve.max_level;
        board.branches = tree
            .branches
            .iter()
            .map(|(id, branch)| (id.clone(), branch.name.clone()))
            .collect();
        board
            .players
            .sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));
        serde_json::to_vec_pretty(&board).map_err(|e| e.to_string())
    }
}
