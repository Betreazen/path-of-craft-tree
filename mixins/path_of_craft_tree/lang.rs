// English for players whose game is not in Russian. The mod's texts are written in Russian; one
// dictionary maps each Russian text to its English one (`data/en-*.json`). A text missing from
// it stays Russian, so a new text never breaks anything, and a harness test keeps the tree's
// texts complete. Game-free: texts.rs holds the game's copy of the dictionary.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Ru,
    En,
}

impl Lang {
    /// The game's language code (`ru_ru`, `en_us`...): Russian for Russian, English for any other.
    /// An unknown (empty) code keeps Russian, as the mod always was.
    pub fn from_code(code: &str) -> Lang {
        let code = code.trim().to_ascii_lowercase();
        if code.is_empty() || code.starts_with("ru") {
            Lang::Ru
        } else {
            Lang::En
        }
    }
}

#[derive(Debug, Default)]
pub struct Dict(HashMap<String, String>);

/// Whether a name's part after `: ` is kept as it is: a number, a percentage, a roman rank, or
/// anything without Russian letters (a block id such as `diamond ore`).
fn is_plain(part: &str) -> bool {
    !part.is_empty() && !part.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё'))
}

impl Dict {
    /// One dictionary from several JSON objects of `"русский": "English"`; a later one wins.
    pub fn parse(parts: &[&str]) -> Result<Dict, String> {
        let mut map = HashMap::new();
        for part in parts {
            let more: HashMap<String, String> =
                serde_json::from_str(part).map_err(|e| e.to_string())?;
            map.extend(more);
        }
        Ok(Dict(map))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The English of `ru`: the text itself; a node name `Основа: уточнение` from its parts, where
    /// a number stays as it is; or a text with numbers in it through its template, the numbers
    /// put back in order (`Ранг 2/5` through `Ранг {}/{}`). `None` when the dictionary does not
    /// know it.
    pub fn english(&self, ru: &str) -> Option<String> {
        if let Some(en) = self.0.get(ru) {
            return Some(en.clone());
        }
        if let Some((base, part)) = ru.split_once(": ") {
            let base = if is_plain(base) {
                Some(base.to_owned())
            } else {
                self.english(base)
            };
            let part = if is_plain(part) {
                Some(part.to_owned())
            } else {
                self.english(part)
            };
            if let (Some(base), Some(part)) = (base, part) {
                return Some(format!("{base}: {part}"));
            }
        }
        let (template, numbers) = template_of(ru);
        if numbers.is_empty() {
            return None;
        }
        let en = self.0.get(&template)?;
        let args: Vec<&dyn Display> = numbers.iter().map(|n| n as &dyn Display).collect();
        Some(fill(en, &args))
    }

    /// `ru` in `lang`, Russian when the dictionary does not know it.
    pub fn text<'a>(&self, lang: Lang, ru: &'a str) -> Cow<'a, str> {
        match lang {
            Lang::Ru => Cow::Borrowed(ru),
            Lang::En => self.english(ru).map_or(Cow::Borrowed(ru), Cow::Owned),
        }
    }

    /// A template with `{}` places, in `lang`, filled with `args` in order.
    pub fn format(&self, lang: Lang, template: &str, args: &[&dyn Display]) -> String {
        fill(&self.text(lang, template), args)
    }
}

/// `template`'s `{}` places filled with `args` in order; a missing argument leaves the place empty.
fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut args = args.iter();
    let mut rest = template;
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        if let Some(arg) = args.next() {
            out.push_str(&arg.to_string());
        }
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// A text's template: each number (digits, with `.` or `,` between digits) and each Latin word
/// (a player's name, a block id) becomes `{}`, and those parts in order. Signs and `%` stay in
/// the template.
fn template_of(text: &str) -> (String, Vec<String>) {
    let mut template = String::with_capacity(text.len());
    let mut numbers = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut i = 0;
    while i < chars.len() {
        if word(chars[i]) {
            let start = i;
            while i < chars.len()
                && (word(chars[i])
                    || (matches!(chars[i], '.' | ',')
                        && chars[i - 1].is_ascii_digit()
                        && chars.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                i += 1;
            }
            numbers.push(chars[start..i].iter().collect());
            template.push_str("{}");
        } else {
            template.push(chars[i]);
            i += 1;
        }
    }
    (template, numbers)
}

/// The English dictionary from the mod's data files (`en-names.json`, `en-texts.json`,
/// `en-ui.json`); texts.rs embeds them in the game, the harness reads them in its tests.
pub fn english(files: &[&[u8]]) -> Result<Dict, String> {
    let parts = files
        .iter()
        .map(|bytes| std::str::from_utf8(bytes).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Dict::parse(&parts)
}

/// A language kept in an atomic, for the client's own language set every frame.
pub struct LangCell(AtomicU8);

impl LangCell {
    pub const fn new() -> LangCell {
        LangCell(AtomicU8::new(0))
    }

    pub fn set(&self, lang: Lang) {
        self.0.store(lang as u8, Ordering::Relaxed);
    }

    pub fn get(&self) -> Lang {
        if self.0.load(Ordering::Relaxed) == Lang::En as u8 {
            Lang::En
        } else {
            Lang::Ru
        }
    }
}

impl Default for LangCell {
    fn default() -> LangCell {
        LangCell::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> Dict {
        Dict::parse(&[
            r#"{"Жила": "Vein", "длина": "length", "Клик: ранг {}/{}": "Click: rank {}/{}"}"#,
        ])
        .unwrap()
    }

    #[test]
    fn only_russian_games_keep_russian() {
        assert_eq!(Lang::from_code("ru_ru"), Lang::Ru);
        assert_eq!(Lang::from_code("RU_RU"), Lang::Ru);
        assert_eq!(Lang::from_code(""), Lang::Ru);
        assert_eq!(Lang::from_code("en_us"), Lang::En);
        assert_eq!(Lang::from_code("de_de"), Lang::En);
    }

    #[test]
    fn names_are_put_together_from_their_parts() {
        let dict = dict();
        assert_eq!(dict.english("Жила").as_deref(), Some("Vein"));
        assert_eq!(dict.english("Жила: длина").as_deref(), Some("Vein: length"));
        assert_eq!(dict.english("Жила: 3").as_deref(), Some("Vein: 3"));
        assert_eq!(dict.english("Жила: 40%").as_deref(), Some("Vein: 40%"));
        assert_eq!(dict.english("Жила: II").as_deref(), Some("Vein: II"));
        assert_eq!(dict.english("Жила: что-то"), None);
        assert_eq!(dict.text(Lang::En, "Незнакомое"), "Незнакомое");
        assert_eq!(dict.text(Lang::Ru, "Жила"), "Жила");
    }

    #[test]
    fn templates_fill_their_places_in_order() {
        let dict = dict();
        let args: [&dyn Display; 2] = [&2, &5];
        assert_eq!(dict.format(Lang::En, "Клик: ранг {}/{}", &args), "Click: rank 2/5");
        assert_eq!(dict.format(Lang::Ru, "Клик: ранг {}/{}", &args), "Клик: ранг 2/5");
        // Fewer arguments than places leave the rest empty, never panic.
        assert_eq!(dict.format(Lang::Ru, "{} и {}", &[&1]), "1 и ");
    }

    #[test]
    fn texts_with_numbers_go_through_their_template() {
        let dict = dict();
        assert_eq!(dict.english("Клик: ранг 2/5").as_deref(), Some("Click: rank 2/5"));
        assert_eq!(dict.english("Клик: ранг 12/15").as_deref(), Some("Click: rank 12/15"));
        let (template, numbers) = template_of("+12.5 опыта · 3,2% и 7");
        assert_eq!(template, "+{} опыта · {}% и {}");
        assert_eq!(numbers, ["12.5", "3,2", "7"]);
        let (template, parts) = template_of("Игрок Steve_1 позвал вас, ждём 2 мин.");
        assert_eq!(template, "Игрок {} позвал вас, ждём {} мин.");
        assert_eq!(parts, ["Steve_1", "2"]);
        assert_eq!(dict.english("Без чисел и неизвестно"), None);
        // A name made of known parts still wins over a template.
        assert_eq!(dict.english("Жила: 3").as_deref(), Some("Vein: 3"));
        // A Latin first part (a player's name, a branch id) stays, the rest is translated.
        assert_eq!(dict.english("Steve: Жила: 2").as_deref(), Some("Steve: Vein: 2"));
    }

    #[test]
    fn the_client_language_cell_starts_russian() {
        let cell = LangCell::new();
        assert_eq!(cell.get(), Lang::Ru);
        cell.set(Lang::En);
        assert_eq!(cell.get(), Lang::En);
    }
}
