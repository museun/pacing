use std::borrow::Cow;

use heck::ToTitleCase as _;

use crate::{
    format::Roman,
    rand::{Rand, SliceExt},
};

pub fn generate_name(max_fragments: impl Into<Option<usize>>, rng: &Rand) -> String {
    #[rustfmt::skip]
    const PARTS: [&[&str]; 3] = [
        ["br", "cr", "dr", "fr", "gr", "j", "kr", "l", "m", "n", "pr", " ", " ", " ", "r", "sh", "tr", "v", "wh", "x", "y", "z"].as_slice(),
        ["a", "a", "e", "e", "i", "i", "o", "o", "u", "u", "ae", "ie", "oo", "ou"].as_slice(),
        ["b", "ck", "d", "g", "k", "m", "n", "p", "t", "v", "x", "z"].as_slice(),
    ];
    (0..max_fragments.into().unwrap_or(6))
        .fold(String::new(), |a, i| a + PARTS[i % 3].choice(rng))
        .to_title_case()
}

pub fn act_name(act: i32) -> String {
    if act == 0 {
        return String::from("Prologue");
    }

    format!("Act {}", Roman::from_i32(act))
}

pub fn plural(subject: &str) -> String {
    match () {
        _ if subject.ends_with('y') => format!("{}ies", &subject[..subject.len() - 1]),
        _ if subject.ends_with("us") => format!("{}i", &subject[..subject.len() - 2]),
        _ if subject.ends_with(['x', 's']) | subject.ends_with("ch") | subject.ends_with("sh") => {
            format!("{subject}es")
        }
        _ if subject.ends_with('f') => format!("{}ves", &subject[..subject.len() - 1]),
        _ if subject.ends_with("man") | subject.ends_with("Man") => {
            format!("{}en", &subject[..subject.len() - 2])
        }
        _ => format!("{subject}s"),
    }
}

pub fn indefinite(subject: &str, quantity: usize) -> String {
    match quantity {
        1 if subject.starts_with(['A', 'E', 'I', 'O', 'U', 'a', 'e', 'i', 'o', 'u']) => {
            format!("an {subject}")
        }
        1 => format!("a {subject}"),
        _ => format!("{quantity} {subject}", subject = plural(subject)),
    }
}

pub fn definite(subject: &str, quantity: usize) -> String {
    let subject = if quantity > 1 {
        Cow::from(plural(subject))
    } else {
        Cow::from(subject)
    };
    format!("the {subject}",)
}

pub fn prefix<'a, 's>(
    list: &[&str],
    m: usize,
    subject: &'a str,
    sep: impl Into<Option<&'s str>>,
) -> Cow<'a, str> {
    if m < 1 || m > list.len() {
        return Cow::from(subject);
    }

    Cow::from(format!(
        "{}{sep}{subject}",
        list[m - 1],
        sep = sep.into().unwrap_or(" ")
    ))
}

pub fn sick(m: usize, subject: &str) -> Cow<'_, str> {
    const LIST: &[&str] = &["dead", "comatose", "crippled", "sick", "undernourished"];
    prefix(LIST, LIST.len().saturating_sub(m), subject, None)
}

pub fn young(m: usize, subject: &str) -> Cow<'_, str> {
    const LIST: &[&str] = &["fetal", "baby", "preadolescent", "teenage", "underage"];
    prefix(LIST, LIST.len().saturating_sub(m), subject, None)
}

pub fn big(m: usize, subject: &str) -> Cow<'_, str> {
    const LIST: &[&str] = &["greater", "massive", "enormous", "giant", "titantic"];
    prefix(LIST, m, subject, None)
}

pub fn special(m: usize, subject: &str) -> Cow<'_, str> {
    if subject.contains(' ') {
        const LIST: &[&str] = &["veteran", "cursed", "warrior", "undead", "demon"];
        prefix(LIST, m, subject, None)
    } else {
        const LIST: &[&str] = &["Battle-", "cursed ", "Were-", "undead ", "demon "];
        prefix(LIST, m, subject, Some(""))
    }
}

pub fn terminate_message(player_name: &str, rng: &Rand) -> String {
    let adjective = ["faithful", "noble", "loyal", "brave"].choice(rng);
    format!("Terminate {adjective} {player_name}?")
}
