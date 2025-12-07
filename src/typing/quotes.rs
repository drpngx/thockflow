// Typing practice quotes
// Loaded from static/quotes.txt

use lazy_static::lazy_static;

const QUOTES_RAW: &str = include_str!("../../static/quotes.txt");

lazy_static! {
    pub static ref QUOTES: Vec<&'static str> = QUOTES_RAW
        .lines()
        .filter(|line| !line.is_empty())
        .collect();
}
