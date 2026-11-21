//! YAML read into `serde_json::Value` (Insomnia's YAML export, OpenAPI and Swagger in YAML) with
//! `yaml-rust2`, pure Rust. Aliases are refused before the parse: the loader copies the anchored
//! node for each alias, so a few lines could grow into gigabytes ("billion laughs"); neither
//! Insomnia nor a usual API description writes one. So is a document nested deeper than
//! `MAX_DEPTH`, counted by the same events: a tree that deep overflows the stack when dropped.

use serde_json::{Map, Number, Value};
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::{Yaml, YamlLoader};

/// The deepest document read.
const MAX_DEPTH: usize = 128;
/// Why a document with an alias is not read.
const ALIAS: &str = "YAML aliases are not read";
/// Why a document nested deeper than [`MAX_DEPTH`] is not read.
const TOO_DEEP: &str = "the YAML document is too deep";

/// The first document of `text`.
///
/// # Errors
///
/// An alias, a document that does not read, too deep, a key that is no scalar.
pub fn parse(text: &str) -> Result<Value, String> {
    check(text)?;
    let documents =
        YamlLoader::load_from_str(text).map_err(|error| format!("not YAML: {error}"))?;
    let first = documents
        .into_iter()
        .next()
        .ok_or_else(|| "an empty YAML document".to_owned())?;
    to_json(&first, 0)
}

/// The document read by the parser's events alone, before anything is built: no alias, no
/// nesting deeper than [`MAX_DEPTH`] (a built tree that deep overflows the stack when dropped).
fn check(text: &str) -> Result<(), String> {
    let mut parser = Parser::new_from_str(text);
    let mut depth = 0usize;
    loop {
        match parser.next_token() {
            Ok((Event::Alias(_), _)) => return Err(ALIAS.to_owned()),
            Ok((Event::SequenceStart(..) | Event::MappingStart(..), _)) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(TOO_DEEP.to_owned());
                }
            }
            Ok((Event::SequenceEnd | Event::MappingEnd, _)) => depth = depth.saturating_sub(1),
            Ok((Event::StreamEnd, _)) => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(format!("not YAML: {error}")),
        }
    }
}

fn key_of(key: &Yaml) -> Result<String, String> {
    match key {
        Yaml::String(text) | Yaml::Real(text) => Ok(text.clone()),
        Yaml::Integer(number) => Ok(number.to_string()),
        Yaml::Boolean(flag) => Ok(flag.to_string()),
        Yaml::Null => Ok("null".to_owned()),
        _ => Err("a YAML key that is no scalar".to_owned()),
    }
}

fn to_json(node: &Yaml, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(TOO_DEEP.to_owned());
    }
    Ok(match node {
        Yaml::Real(text) => text
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map_or_else(|| Value::String(text.clone()), Value::Number),
        Yaml::Integer(number) => Value::from(*number),
        Yaml::String(text) => Value::String(text.clone()),
        Yaml::Boolean(flag) => Value::Bool(*flag),
        Yaml::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| to_json(item, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Yaml::Hash(map) => {
            let mut out = Map::new();
            for (key, value) in map {
                out.insert(key_of(key)?, to_json(value, depth + 1)?);
            }
            Value::Object(out)
        }
        Yaml::Null | Yaml::Alias(_) | Yaml::BadValue => Value::Null,
    })
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
