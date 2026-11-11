// What other mods' machines report to the tree, free of game types. A machine mod (RustCraft)
// runs the console command `pct-machine <report>` through `native_run_mod_command`; without this
// mod the command is simply unknown, so neither mod needs the other to build. The contract for
// machine mods is docs/machine-reports.md.

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

    fn int(&mut self, what: &str) -> Result<i32, String> {
        let word = self.word(what)?;
        word.parse::<i32>()
            .map_err(|_| format!("{what} {word} is not a whole number"))
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
    /// The machine slaughtered farm animal `entity` of `kind` in a pen: husbandry, not combat.
    Slaughter {
        owner: u128,
        entity: u32,
        kind: String,
    },
    /// A player's own ranged weapon from the machine mod (an energy rifle) killed mob `entity`:
    /// the player earns in full, as for a bow, since the shot was theirs.
    Shot {
        player: u128,
        entity: u32,
        kind: String,
    },
    /// The machine caught `count` of `item`; the tree's lists say fish, treasure or junk.
    Fish {
        owner: u128,
        item: String,
        count: u32,
    },
    /// The machine milked cow `entity`, which then rests as after a player's bucket.
    Milk { owner: u128, entity: u32 },
    /// The machine placed `block` at `pos` as part of a building: it is marked as placed and pays
    /// the building branch where nobody built in the last day, as a player's block does.
    Build {
        owner: u128,
        dimension: String,
        pos: [i32; 3],
        block: String,
    },
    /// The machine entered chunk `x`, `z` (chunk coordinates) of `dimension`.
    Chunk {
        owner: u128,
        dimension: String,
        x: i32,
        z: i32,
    },
    /// The machine found `biome`.
    Biome { owner: u128, biome: String },
    /// The machine found `structure`.
    Structure { owner: u128, structure: String },
    /// Work paid by one action of the tree, `times` over: repairs, breeding, shearing, honey,
    /// brushing, brews and enchanting levels.
    Work { owner: u128, work: Work, times: u32 },
}

/// The kinds of work a [`Report::Work`] counts, each with the word it is sent as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// Count: items repaired.
    Repair,
    /// Count: babies born.
    Breed,
    /// Count: animals sheared.
    Shear,
    /// Count: hives emptied.
    Honey,
    /// Count: suspicious blocks brushed.
    Brush,
    /// Count: brewing runs finished, each a stand of three bottles.
    Brew,
    /// Count: enchanting levels charged, as a table would charge them (1 to 3 an item).
    Enchant,
}

impl Work {
    const ALL: [(&'static str, Work); 7] = [
        ("repair", Work::Repair),
        ("breed", Work::Breed),
        ("shear", Work::Shear),
        ("honey", Work::Honey),
        ("brush", Work::Brush),
        ("brew", Work::Brew),
        ("enchant", Work::Enchant),
    ];

    fn from_word(word: &str) -> Option<Work> {
        Self::ALL
            .iter()
            .find(|(name, _)| *name == word)
            .map(|(_, work)| *work)
    }
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
        "slaughter" => Report::Slaughter {
            owner: words.owner()?,
            entity: words.whole("entity")?,
            kind: words.text("kind")?,
        },
        "shot" => Report::Shot {
            player: words.owner()?,
            entity: words.whole("entity")?,
            kind: words.text("kind")?,
        },
        "fish" => Report::Fish {
            owner: words.owner()?,
            item: words.text("item")?,
            count: words.count("count")?,
        },
        "milk" => Report::Milk {
            owner: words.owner()?,
            entity: words.whole("entity")?,
        },
        "build" => Report::Build {
            owner: words.owner()?,
            dimension: words.text("dimension")?,
            pos: words.pos()?,
            block: words.text("block")?,
        },
        "chunk" => Report::Chunk {
            owner: words.owner()?,
            dimension: words.text("dimension")?,
            x: words.int("chunk x")?,
            z: words.int("chunk z")?,
        },
        "biome" => Report::Biome {
            owner: words.owner()?,
            biome: words.text("biome")?,
        },
        "structure" => Report::Structure {
            owner: words.owner()?,
            structure: words.text("structure")?,
        },
        other => match Work::from_word(other) {
            Some(work) => Report::Work {
                owner: words.owner()?,
                work,
                times: words.count("count")?,
            },
            None => return Err(format!("unknown report {other}")),
        },
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
    fn shot_fish_milk_and_build_reports_read_back() {
        assert_eq!(
            parse("slaughter 0123abcd456789ef0123456789abcdef 88 minecraft:cow"),
            Ok(Report::Slaughter {
                owner: OWNER,
                entity: 88,
                kind: "minecraft:cow".to_owned(),
            })
        );
        assert_eq!(
            parse("shot 0123abcd456789ef0123456789abcdef 77 minecraft:skeleton"),
            Ok(Report::Shot {
                player: OWNER,
                entity: 77,
                kind: "minecraft:skeleton".to_owned(),
            })
        );
        assert_eq!(
            parse("fish 0123abcd456789ef0123456789abcdef minecraft:cod 2"),
            Ok(Report::Fish {
                owner: OWNER,
                item: "minecraft:cod".to_owned(),
                count: 2,
            })
        );
        assert_eq!(
            parse("milk 0123abcd456789ef0123456789abcdef 512"),
            Ok(Report::Milk {
                owner: OWNER,
                entity: 512,
            })
        );
        assert_eq!(
            parse(
                "build 0123abcd456789ef0123456789abcdef minecraft:overworld 4 70 -8 \
                 minecraft:stone_bricks"
            ),
            Ok(Report::Build {
                owner: OWNER,
                dimension: "minecraft:overworld".to_owned(),
                pos: [4, 70, -8],
                block: "minecraft:stone_bricks".to_owned(),
            })
        );
    }

    #[test]
    fn discovery_reports_read_back() {
        assert_eq!(
            parse("chunk 0123abcd456789ef0123456789abcdef minecraft:the_nether -3 12"),
            Ok(Report::Chunk {
                owner: OWNER,
                dimension: "minecraft:the_nether".to_owned(),
                x: -3,
                z: 12,
            })
        );
        assert_eq!(
            parse("biome 0123abcd456789ef0123456789abcdef minecraft:cherry_grove"),
            Ok(Report::Biome {
                owner: OWNER,
                biome: "minecraft:cherry_grove".to_owned(),
            })
        );
        assert_eq!(
            parse("structure 0123abcd456789ef0123456789abcdef minecraft:village_plains"),
            Ok(Report::Structure {
                owner: OWNER,
                structure: "minecraft:village_plains".to_owned(),
            })
        );
    }

    #[test]
    fn every_work_word_reads_back_with_its_count() {
        for (word, work) in Work::ALL {
            assert_eq!(
                parse(&format!("{word} 0123abcd456789ef0123456789abcdef 3")),
                Ok(Report::Work {
                    owner: OWNER,
                    work,
                    times: 3,
                }),
                "{word}"
            );
        }
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
            "brew 0123abcd456789ef0123456789abcdef 0",
            "honey 0123abcd456789ef0123456789abcdef",
            "enchant 0123abcd456789ef0123456789abcdef 3 minecraft:book",
            "milk 0123abcd456789ef0123456789abcdef",
            "chunk 0123abcd456789ef0123456789abcdef minecraft:overworld 1.5 2",
            "biome 0123abcd456789ef0123456789abcdef",
            "shot 0123abcd456789ef0123456789abcdef -4 minecraft:zombie",
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
