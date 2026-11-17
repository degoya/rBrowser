//! Running one command or host tool per target: `target` resolved against the views (a
//! breakpoint id names every view showing it) and handed on as `"all"` or a view id, each call
//! once per resolved target. Without `target` the call goes out without one, so the command or
//! the host tool takes its own default (the active view, or every view).

use rb_engine::Target;
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve};

/// What runs a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Via {
    /// A command of the one registry (`Backend::run_command`).
    Command,
    /// A tool the host runs itself (`Backend::host_tool`).
    Host,
}

/// How the answers of several calls (a breakpoint id shown by several views) become one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answers {
    /// The lists of each answer joined (`views`, `shots`, `moved`, ...), the rest the last one's.
    Joined,
    /// The last answer: it is a state, not a list of results.
    Last,
    /// As [`Answers::Joined`], and the counts both have (`regressions`, ...) added up.
    Totals,
}

impl RbMcpServer {
    /// Runs the command `id` once per target of `target` with `args` and the target.
    pub(crate) async fn per_target(
        &self,
        id: &str,
        target: Option<&TargetArg>,
        args: Value,
    ) -> Result<Value, ToolFailure> {
        self.fan_out(Via::Command, id, target, args, Answers::Joined, None)
            .await
    }

    /// Runs the host tool `name` once per target of `target` with `args` and the target.
    pub(crate) async fn host_per_target(
        &self,
        name: &str,
        target: Option<&TargetArg>,
        args: Value,
        answers: Answers,
    ) -> Result<Value, ToolFailure> {
        self.fan_out(Via::Host, name, target, args, answers, None)
            .await
    }

    /// [`Self::host_per_target`] with the inline images (`pngBase64`) of every call together
    /// bound to `limit` bytes (`mcp.maxAnswerBytes`): over it, the fan-out stops with
    /// [`ANSWER_TOO_LARGE`].
    pub(crate) async fn host_per_target_inline(
        &self,
        name: &str,
        target: Option<&TargetArg>,
        args: Value,
        limit: usize,
    ) -> Result<Value, ToolFailure> {
        self.fan_out(Via::Host, name, target, args, Answers::Joined, Some(limit))
            .await
    }

    async fn fan_out(
        &self,
        via: Via,
        name: &str,
        target: Option<&TargetArg>,
        args: Value,
        answers: Answers,
        inline_limit: Option<usize>,
    ) -> Result<Value, ToolFailure> {
        let targets: Vec<Option<Target>> = match target {
            None => vec![None],
            Some(target) => {
                let views = self.read(|backend| backend.views()).await?;
                resolve(Some(target), &views)?
                    .targets()
                    .into_iter()
                    .map(Some)
                    .collect()
            }
        };
        let mut answer = Value::Null;
        let mut inline = 0;
        for target in targets {
            let call = with_target(&args, target)?;
            let part = match via {
                Via::Command => self.command(name, call).await?,
                Via::Host => self.backend.host_tool(name, call).await?,
            };
            if let Some(limit) = inline_limit {
                inline += inline_bytes(&part);
                if inline > limit {
                    return Err(too_large(limit));
                }
            }
            answer = match answers {
                Answers::Joined => joined(answer, part),
                Answers::Last => part,
                Answers::Totals => totals(answer, part),
            };
        }
        Ok(answer)
    }
}

/// The code of an answer over `mcp.maxAnswerBytes` that cannot be cut.
pub(crate) const ANSWER_TOO_LARGE: &str = "answer_too_large";

fn too_large(limit: usize) -> ToolFailure {
    ToolFailure::new(
        ANSWER_TOO_LARGE,
        format!(
            "the images are larger than mcp.maxAnswerBytes ({limit} bytes) together; take them \
             with save: true and read the files, or capture fewer views"
        ),
    )
    .with_setting("mcp.maxAnswerBytes")
}

/// The bytes of every `pngBase64` string in `value`, at any depth.
fn inline_bytes(value: &Value) -> usize {
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, child)| match child {
                Value::String(text) if key == "pngBase64" => text.len(),
                _ => inline_bytes(child),
            })
            .sum(),
        Value::Array(items) => items.iter().map(inline_bytes).sum(),
        _ => 0,
    }
}

/// `args` (an object; anything else is taken as none) with `target` set, when there is one.
pub(crate) fn with_target(args: &Value, target: Option<Target>) -> Result<Value, ToolFailure> {
    let mut call = match args {
        Value::Object(fields) => Value::Object(fields.clone()),
        _ => json!({}),
    };
    if let Some(target) = target {
        call["target"] = serde_json::to_value(target)
            .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
    }
    Ok(call)
}

/// `part` added to `answer`: a list under a key both have is joined, anything else is `part`'s.
pub(crate) fn joined(answer: Value, part: Value) -> Value {
    let (Value::Object(mut answer), Value::Object(part)) = (answer, part.clone()) else {
        return part;
    };
    for (key, value) in part {
        if let (Some(Value::Array(items)), Value::Array(more)) = (answer.get_mut(&key), &value) {
            items.extend(more.iter().cloned());
            continue;
        }
        answer.insert(key, value);
    }
    Value::Object(answer)
}

/// [`joined`], and an unsigned count under a key both have added up.
pub(crate) fn totals(answer: Value, part: Value) -> Value {
    let counts: Vec<(String, u64)> = match (&answer, &part) {
        (Value::Object(before), Value::Object(now)) => now
            .iter()
            .filter_map(|(key, value)| {
                let sum = before.get(key)?.as_u64()?.checked_add(value.as_u64()?)?;
                Some((key.clone(), sum))
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut answer = joined(answer, part);
    for (key, sum) in counts {
        answer[key.as_str()] = Value::from(sum);
    }
    answer
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod tests;
