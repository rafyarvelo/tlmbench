//! Recipes: a named expression with a unit, category and decimals, kept in one library and shared as
//! `.tlmbench/recipes/<recipe>.toml`. A recipe can use other recipes by name.

use serde::{Deserialize, Serialize};

use crate::ast::{Expr, ExprError};
use crate::parse;

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub expr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// "Any file with these channels" gets it without being asked.
    #[serde(default = "yes")]
    pub automatic: bool,
}

#[derive(Serialize, Deserialize)]
struct File {
    schema: u32,
    #[serde(default)]
    recipe: Vec<Recipe>,
}

pub const SCHEMA: u32 = 1;

/// The recipes in a recipe file's text.
pub fn from_toml(text: &str) -> Result<Vec<Recipe>, String> {
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    if f.schema != SCHEMA {
        return Err(format!("recipe file schema {} isn't supported (this version reads {SCHEMA})", f.schema));
    }
    Ok(f.recipe)
}

/// A recipe file's text.
pub fn to_toml(recipes: &[Recipe]) -> String {
    toml::to_string(&File { schema: SCHEMA, recipe: recipes.to_vec() }).expect("recipes serialize")
}

/// A recipe ready to evaluate: its expression with the recipes it uses substituted in.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub recipe: Recipe,
    pub expr: Expr,
}

impl Resolved {
    /// 32 hex digits that change when anything about the result changes: the expanded expression, name, unit,
    /// category and decimals.
    pub fn key(&self) -> String {
        let r = &self.recipe;
        let text = format!("{:?}\n{}\n{:?}\n{:?}\n{:?}", self.expr, r.name, r.unit, r.category, r.decimals);
        blake3::hash(text.as_bytes()).to_hex()[..32].to_owned()
    }
}

/// Resolves each recipe against the channels a file has (`has_channel`, an exact name): the recipes it uses are
/// substituted in (a channel wins over a recipe of the same name), and each name left must be a channel.
/// Recipes that can't be resolved come back with the reason: they don't apply to this file.
pub fn resolve(recipes: &[Recipe], has_channel: &dyn Fn(&str) -> bool) -> (Vec<Resolved>, Vec<(String, ExprError)>) {
    let (mut ok, mut failed) = (Vec::new(), Vec::new());
    for r in recipes {
        match expand(r, recipes, has_channel, &mut vec![r.name.clone()]) {
            Ok(expr) => ok.push(Resolved { recipe: r.clone(), expr }),
            Err(e) => failed.push((r.name.clone(), e)),
        }
    }
    (ok, failed)
}

fn expand(
    r: &Recipe,
    all: &[Recipe],
    has_channel: &dyn Fn(&str) -> bool,
    stack: &mut Vec<String>,
) -> Result<Expr, ExprError> {
    let tree = parse(&r.expr)?;
    let mut err = None;
    let out = tree.map_names(&mut |name, at| {
        if err.is_some() || name.split('|').map(str::trim).any(has_channel) {
            return None;
        }
        // not a channel: another recipe?
        let Some(other) = all.iter().find(|o| name.split('|').map(str::trim).any(|alt| alt == o.name)) else {
            err = Some(ExprError::new(format!("There's no channel called {name}"), at));
            return None;
        };
        if stack.contains(&other.name) {
            err = Some(ExprError::new(format!("{} uses itself through {}", other.name, r.name), at));
            return None;
        }
        stack.push(other.name.clone());
        let sub = expand(other, all, has_channel, stack);
        stack.pop();
        match sub {
            Ok(e) => Some(e),
            Err(e) => {
                err = Some(ExprError::new(format!("{}: {}", other.name, e.msg), at));
                None
            }
        }
    });
    err.map_or(Ok(out), Err)
}
