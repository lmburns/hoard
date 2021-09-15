//! Select GPG key in keychain

use super::Key;
use colored::Colorize;
use std::collections::HashMap;

/// Select key.
#[must_use]
pub fn select_key<'a>(keys: &'a [Key], prompt: Option<&'a str>) -> Option<&'a Key> {
    let map: HashMap<_, _> = keys.iter().map(|key| (key.to_string(), key)).collect();
    let items: Vec<_> = map.keys().collect();
    select_item(
        prompt.unwrap_or(&format!("{}", "Select key".green().bold())),
        &items,
    )
    .as_ref()
    .map(|item| map[item])
}

/// Interactively select one of the given items.
fn select_item<'a, S: AsRef<str>>(prompt: &'a str, items: &'a [S]) -> Option<String> {
    // Build sorted list of string references as items
    let mut items = items.iter().map(AsRef::as_ref).collect::<Vec<_>>();
    items.sort_unstable();

    loop {
        // Print options and prompt
        items
            .iter()
            .enumerate()
            .for_each(|(i, item)| eprintln!("{}: {}", i + 1, item));
        eprint!("{} (number/empty): ", prompt);

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("failed to read user input from stdin");

        // If empty, we selected none
        if input.trim().is_empty() {
            return None;
        }

        // Try to parse number, select item, or show error and retry
        match input.trim().parse::<usize>().ok() {
            Some(n) if n > 0 && n <= items.len() => return Some(items[n - 1].into()),
            _ => tracing::error!("invalid selection input"),
        }
    }
}
