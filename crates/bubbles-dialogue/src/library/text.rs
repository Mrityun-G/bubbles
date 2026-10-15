//! Text-shaping built-in functions: plural, select.
//!
//! Neither function localises anything. `plural` applies one hard-coded
//! two-form rule, and `select` is a lookup into a separator-delimited string
//! with no escape mechanism. The limitations are recorded on each registration
//! below, in the guide's Expressions chapter, and on [`FunctionLibrary`], which
//! is where a host replaces either of them.

use crate::error::DialogueError;
use crate::value::Value;

use super::FunctionLibrary;

impl FunctionLibrary {
    pub(super) fn register_text_builtins(&mut self) {
        // `select(key, "k1:a|k2:b|other:c")` splits the mapping on `|`, then
        // each entry at its first `:`. Neither separator can be escaped, so a
        // key containing `|` or `:` can never be matched and a value containing
        // `|` cannot be expressed; a value may contain `:`, because only the
        // first colon in an entry separates. The mapping is parsed on every
        // call.
        self.register("select", |args| match args.as_slice() {
            [Value::Text(key), Value::Text(mapping)] => {
                let mut fallback: Option<&str> = None;
                for entry in mapping.split('|') {
                    let (k, v) = entry
                        .split_once(':')
                        .ok_or_else(|| DialogueError::Function {
                            name: "select".into(),
                            message: format!("mapping entry {entry:?} has no ':' separator"),
                        })?;
                    if k == "other" {
                        fallback = Some(v);
                    } else if k == key.as_str() {
                        return Ok(Value::Text(v.to_owned()));
                    }
                }
                fallback
                    .map(|v| Value::Text(v.to_owned()))
                    .ok_or_else(|| DialogueError::Function {
                        name: "select".into(),
                        message: format!(
                            "no match for key {key:?} and no 'other' fallback in mapping"
                        ),
                    })
            }
            _ => Err(DialogueError::Function {
                name: "select".into(),
                message: format!("expected (string, string), got {args:?}"),
            }),
        });
        // `plural(n, singular, plural)` returns the singular form when `|n|` is
        // 1 (compared within `f64::EPSILON`) and the plural form for every
        // other value, 0 included. That is a two-form rule, which fits English
        // and the languages that share its shape and is wrong everywhere else:
        // Polish, Russian, Czech and Arabic have three to six plural
        // categories, and French takes the singular for 0. A CLDR-correct
        // implementation belongs in the host, registered under this name
        // through `FunctionLibrary::register`.
        self.register("plural", |args| match args.as_slice() {
            [Value::Number(n), Value::Text(singular), Value::Text(plural)] => {
                let form = if (n.abs() - 1.0).abs() < f64::EPSILON {
                    singular
                } else {
                    plural
                };
                Ok(Value::Text(form.clone()))
            }
            _ => Err(DialogueError::Function {
                name: "plural".into(),
                message: format!("expected (number, string, string), got {args:?}"),
            }),
        });
    }
}
