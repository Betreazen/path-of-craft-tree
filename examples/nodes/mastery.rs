// Mastery after a branch's last level (docs/design.md, section 16): a slow scale that keeps
// counting the branch's experience, pays no points and only gives a rank and a title.
use serde_json::{Value, json};

/// Rank r costs floor(COEFFICIENT · r^EXPONENT) experience after the last level: the first rank
/// comes about 1,7 hours after level 75 at the 3 565 an hour the pace targets, the tenth after
/// about 30 more, all ten after about 148. Each rank takes a little longer than the one before.
const COEFFICIENT: f64 = 6000.0;
const EXPONENT: f64 = 1.25;
pub const MAX_RANK: u32 = 10;

/// Titles of ranks 1 to 9; rank 10 is each branch's own legend.
const TITLES: [&str; 9] = [
    "Знаток",
    "Умелец",
    "Мастер",
    "Старший мастер",
    "Наставник",
    "Магистр",
    "Виртуоз",
    "Мэтр",
    "Грандмастер",
];

pub fn mastery() -> Value {
    json!({
        "curve": {"coefficient": COEFFICIENT, "exponent": EXPONENT, "max_level": MAX_RANK},
        "titles": TITLES,
    })
}

/// The title of a branch's highest mastery rank.
pub fn legend(branch: &str) -> &'static str {
    match branch {
        "mining" => "Владыка недр",
        "digging" => "Повелитель земли",
        "woodcutting" => "Хозяин леса",
        "fishing" => "Морской волк",
        "farming" => "Кормилец края",
        "husbandry" => "Пастырь стад",
        "cooking" => "Великий кулинар",
        "alchemy" => "Хранитель эликсиров",
        "building" => "Зодчий",
        "crafting" => "Золотые руки",
        "smithing" => "Повелитель горна",
        "enchanting" => "Архимаг",
        "melee" => "Гроза чудовищ",
        "archery" => "Соколиный глаз",
        "defense" => "Несокрушимый",
        "exploration" => "Покоритель миров",
        other => panic!("no mastery legend for {other}"),
    }
}
