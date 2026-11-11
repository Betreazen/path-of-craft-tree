// What other mods' machines report to the tree, free of game types. A machine mod (RustCraft)
// runs the console command `pct-machine <report>` through `native_run_mod_command`; without this
// mod the command is simply unknown, so neither mod needs the other to build. The contract is
// written for the machine mod in RustCraft's docs/design/path-of-craft-tree-contract.md.

/// The console command the reports arrive by.
pub const COMMAND: &str = "pct-machine";

/// The tool a machine works like: it picks the branch a broken block pays, as a player's tool does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Hand,
}

impl Tool {
    fn parse(word: &str) -> Result<Tool, String> {
        match word {
            "pickaxe" => Ok(Tool::Pickaxe),
            "axe" => Ok(Tool::Axe),
            "shovel" => Ok(Tool::Shovel),
            "hoe" => Ok(Tool::Hoe),
            "hand" => Ok(Tool::Hand),
            _ => Err(format!("unknown tool {word}")),
        }
    }

    /// A stand-in item id ending like the tools the branches ask for (`_pickaxe`, `_axe`...).
    pub fn item(self) -> &'static str {
        match self {
            Tool::Pickaxe => "pct:machine_pickaxe",
            Tool::Axe => "pct:machine_axe",
            Tool::Shovel => "pct:machine_shovel",
            Tool::Hoe => "pct:machine_hoe",
            Tool::Hand => "pct:machine",
        }
    }
}

/// The words of a report, read one by one with a reason for whatever is missing or wrong.
struct Words<'a>(std::str::SplitWhitespace<'a>);

impl<'a> Words<'a> {
    fn word(&mut self, what: &str) -> Result<&'a str, String> {
        self.0.next().ok_or_else(|| format!("missing {what}"))
    }

    fn owner(&mut self) -> Result<u128, String> {
        let word = self.word("owner")?;
        let hex: String = word.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return Err(format!("owner {word} is not a UUID"));
        }
        u128::from_str_radix(&hex, 16).map_err(|_| format!("owner {word} is not a UUID"))
    }

    fn pos(&mut self) -> Result<[i32; 3], String> {
        let mut axis = |name| {
            let word = self.word(name)?;
            word.parse::<i32>()
                .map_err(|_| format!("{name} {word} is not a whole number"))
        };
        Ok([axis("x")?, axis("y")?, axis("z")?])
    }

    fn whole(&mut self, what: &str) -> Result<u32, String> {
        let word = self.word(what)?;
        word.parse::<u32>()
            .map_err(|_| format!("{what} {word} is not a whole number"))
    }

    /// A count of at least one.
    fn count(&mut self, what: &str) -> Result<u32, String> {
        let word = self.word(what)?;
        word.parse::<u32>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| format!("{what} {word} is not a positive whole number"))
    }

    fn text(&mut self, what: &str) -> Result<String, String> {
        self.word(what).map(str::to_owned)
    }

    fn end(mut self, report: Report) -> Result<Report, String> {
        match self.0.next() {
            None => Ok(report),
            Some(extra) => Err(format!("unexpected {extra} at the end")),
        }
    }
}

/// One thing a machine did.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// The machine broke `block` at `pos`; its owner earns as if they broke it with `tool`.
    Break {
        owner: u128,
        dimension: String,
        pos: [i32; 3],
        block: String,
        tool: Tool,
    },
    /// The machine put `block` at `pos`: like a player's block, it earns nothing when broken.
    Place {
        dimension: String,
        pos: [i32; 3],
        block: String,
    },
    /// The machine took `count` of `item` out of its furnace.
    Smelt {
        owner: u128,
        item: String,
        count: u32,
    },
    /// The machine ran `recipe` `crafts` times, making `count` of `item` in all.
    Craft {
        owner: u128,
        recipe: String,
        item: String,
        count: u32,
        crafts: u32,
    },
    /// The machine killed mob `entity` of `kind`.
    Kill {
        owner: u128,
        entity: u32,
        kind: String,
    },
}

/// Reads the words after the command name.
pub fn parse(line: &str) -> Result<Report, String> {
    let mut words = Words(line.split_whitespace());
    let report = match words.word("report kind")? {
        "break" => Report::Break {
            owner: words.owner()?,
            dimension: words.text("dimension")?,
            pos: words.pos()?,
            block: words.text("block")?,
            tool: Tool::parse(words.word("tool")?)?,
        },
        "place" => Report::Place {
            dimension: words.text("dimension")?,
            pos: words.pos()?,
            block: words.text("block")?,
        },
        "smelt" => Report::Smelt {
            owner: words.owner()?,
            item: words.text("item")?,
            count: words.count("count")?,
        },
        "craft" => Report::Craft {
            owner: words.owner()?,
            recipe: words.text("recipe")?,
            item: words.text("item")?,
            count: words.count("count")?,
            crafts: words.count("crafts")?,
        },
        "kill" => Report::Kill {
            owner: words.owner()?,
            entity: words.whole("entity")?,
            kind: words.text("kind")?,
        },
        other => return Err(format!("unknown report {other}")),
    };
    words.end(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 0x0123_abcd_4567_89ef_0123_4567_89ab_cdef;

    #[test]
    fn a_break_report_names_owner_place_block_and_tool() {
        let report = parse(
            "break 0123abcd-4567-89ef-0123-456789abcdef minecraft:overworld 10 -59 -3 \
             minecraft:iron_ore pickaxe",
        );
        assert_eq!(
            report,
            Ok(Report::Break {
                owner: OWNER,
                dimension: "minecraft:overworld".to_owned(),
                pos: [10, -59, -3],
                block: "minecraft:iron_ore".to_owned(),
                tool: Tool::Pickaxe,
            })
        );
    }

    #[test]
    fn the_owner_may_be_written_with_or_without_dashes() {
        let plain = parse("smelt 0123abcd456789ef0123456789abcdef minecraft:iron_ingot 2");
        let dashed = parse("smelt 0123abcd-4567-89ef-0123-456789abcdef minecraft:iron_ingot 2");
        assert_eq!(plain, dashed);
        assert_eq!(
            plain,
            Ok(Report::Smelt {
                owner: OWNER,
                item: "minecraft:iron_ingot".to_owned(),
                count: 2
            })
        );
    }

    #[test]
    fn place_craft_and_kill_reports_read_back() {
        assert_eq!(
            parse("place minecraft:the_nether 1 2 3 minecraft:netherrack"),
            Ok(Report::Place {
                dimension: "minecraft:the_nether".to_owned(),
                pos: [1, 2, 3],
                block: "minecraft:netherrack".to_owned(),
            })
        );
        assert_eq!(
            parse("craft 0123abcd456789ef0123456789abcdef minecraft:paper minecraft:paper 9 3"),
            Ok(Report::Craft {
                owner: OWNER,
                recipe: "minecraft:paper".to_owned(),
                item: "minecraft:paper".to_owned(),
                count: 9,
                crafts: 3,
            })
        );
        assert_eq!(
            parse("kill 0123abcd456789ef0123456789abcdef 4021 minecraft:zombie"),
            Ok(Report::Kill {
                owner: OWNER,
                entity: 4021,
                kind: "minecraft:zombie".to_owned(),
            })
        );
    }

    #[test]
    fn every_tool_word_is_known_and_others_are_refused() {
        for (word, tool) in [
            ("pickaxe", Tool::Pickaxe),
            ("axe", Tool::Axe),
            ("shovel", Tool::Shovel),
            ("hoe", Tool::Hoe),
            ("hand", Tool::Hand),
        ] {
            assert_eq!(Tool::parse(word), Ok(tool));
        }
        assert!(Tool::parse("drill").is_err());
    }

    #[test]
    fn broken_reports_are_refused_with_a_reason() {
        for line in [
            "",
            "dance 0123abcd456789ef0123456789abcdef",
            "smelt nobody minecraft:iron_ingot 2",
            "smelt 0123abcd456789ef0123456789abcdef minecraft:iron_ingot",
            "smelt 0123abcd456789ef0123456789abcdef minecraft:iron_ingot -1",
            "smelt 0123abcd456789ef0123456789abcdef minecraft:iron_ingot 0",
            "break 0123abcd456789ef0123456789abcdef minecraft:overworld 1 2 minecraft:stone pickaxe",
            "kill 0123abcd456789ef0123456789abcdef 4021 minecraft:zombie extra",
            "craft 0123abcd456789ef0123456789abcdef minecraft:paper minecraft:paper 9 0",
        ] {
            assert!(parse(line).is_err(), "accepted {line:?}");
        }
    }

    #[test]
    fn the_tool_word_gives_a_tool_name_with_the_branch_suffix() {
        assert!(Tool::Pickaxe.item().ends_with("_pickaxe"));
        assert!(Tool::Axe.item().ends_with("_axe"));
        assert!(Tool::Shovel.item().ends_with("_shovel"));
        assert!(Tool::Hoe.item().ends_with("_hoe"));
        assert!(!Tool::Hand.item().ends_with("axe"));
    }
}
