//! A Bruno `.bru` file's blocks: `name {` ... `}` of `key: value` lines (`~` before a disabled
//! one) or of text indented by two spaces, and `name [` ... `]` lists ([`crate::bruno`] reads
//! what they mean).

use rb_api_model::Entry;

/// One block of a `.bru` file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    /// `key: value` lines: key, value, enabled.
    pub dict: Vec<(String, String, bool)>,
    /// A text block's text, two spaces of indentation removed.
    pub text: String,
    /// A list block's items.
    pub list: Vec<String>,
}

pub(crate) const METHODS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "trace", "connect",
];

fn is_text(name: &str) -> bool {
    name == "docs"
        || name == "tests"
        || name.starts_with("script:")
        || matches!(
            name,
            "body:json"
                | "body:text"
                | "body:xml"
                | "body:sparql"
                | "body:graphql"
                | "body:graphql:vars"
        )
}

/// The blocks of a `.bru` file.
pub(crate) fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let head = line.trim_end();
        if head.is_empty() || head.starts_with(char::is_whitespace) {
            continue;
        }
        let (name, close) = if let Some(name) = head.strip_suffix('{') {
            (name.trim(), "}")
        } else if let Some(name) = head.strip_suffix('[') {
            (name.trim(), "]")
        } else {
            continue;
        };
        let mut body = Vec::new();
        for line in lines.by_ref() {
            if line.trim_end() == close && !line.starts_with(char::is_whitespace) {
                break;
            }
            body.push(line);
        }
        let mut block = Block {
            name: name.to_owned(),
            ..Block::default()
        };
        if close == "]" {
            block.list = body
                .iter()
                .map(|item| item.trim().trim_end_matches(',').trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect();
        } else if is_text(name) {
            let lines: Vec<&str> = body
                .iter()
                .map(|line| line.strip_prefix("  ").unwrap_or_else(|| line.trim_start()))
                .collect();
            block.text = lines.join("\n").trim_matches('\n').trim_end().to_owned();
        } else {
            block.dict = body.iter().filter_map(|line| dict_line(line)).collect();
        }
        out.push(block);
    }
    out
}

fn dict_line(line: &str) -> Option<(String, String, bool)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (enabled, line) = match line.strip_prefix('~') {
        Some(rest) => (false, rest),
        None => (true, line),
    };
    let (key, value) = line.split_once(':')?;
    Some((key.trim().to_owned(), value.trim().to_owned(), enabled))
}

pub(crate) fn block<'a>(blocks: &'a [Block], name: &str) -> Option<&'a Block> {
    blocks.iter().find(|block| block.name == name)
}

/// The enabled value of `key` in block `name`, empty when there is none.
pub(crate) fn value(blocks: &[Block], name: &str, key: &str) -> String {
    block(blocks, name)
        .and_then(|block| block.dict.iter().find(|(k, _, on)| k == key && *on))
        .map(|(_, value, _)| value.clone())
        .unwrap_or_default()
}

pub(crate) fn entries(blocks: &[Block], name: &str) -> Vec<Entry> {
    block(blocks, name).map_or_else(Vec::new, |block| {
        block
            .dict
            .iter()
            .map(|(key, value, enabled)| Entry {
                name: key.clone(),
                value: value.clone(),
                enabled: *enabled,
            })
            .collect()
    })
}
