//! The recipe library: one file per recipe in the app cache (`recipes/<name>.toml`, the same format Share writes to
//! `.tlmbench/recipes/`), and the check the Derive dialog runs as you type.

use std::fs;
use std::path::{Path, PathBuf};

use tlmbench_core::Recording;
use tlmbench_expr::{Env, ExprError, Recipe, Series};

use crate::derived::RecEnv;
use crate::{Error, Result};

/// The recipes the app keeps, in a folder.
#[derive(Clone, Debug)]
pub struct Library {
    dir: PathBuf,
}

/// A file name for a recipe: its name's letters and digits, lower case, joined by dashes.
fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "recipe".into() } else { s }
}

impl Library {
    /// The library under the cache root `root`.
    pub fn new(root: &Path) -> Self {
        Self { dir: root.join("recipes") }
    }

    fn files(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        v.sort();
        v
    }

    /// Every recipe, by name. A file that doesn't parse is skipped (it's reported by [`Library::problems`]).
    pub fn list(&self) -> Vec<Recipe> {
        let mut out: Vec<Recipe> = self
            .files()
            .iter()
            .filter_map(|f| fs::read_to_string(f).ok())
            .filter_map(|t| tlmbench_expr::recipe::from_toml(&t).ok())
            .flatten()
            .collect();
        out.sort_by_key(|r| r.name.to_lowercase());
        out.dedup_by(|a, b| a.name == b.name);
        out
    }

    /// Files in the library that can't be read as recipes, with why.
    pub fn problems(&self) -> Vec<(PathBuf, String)> {
        self.files()
            .into_iter()
            .filter_map(|f| {
                let text = fs::read_to_string(&f).map_err(|e| e.to_string());
                text.and_then(|t| tlmbench_expr::recipe::from_toml(&t)).err().map(|e| (f, e))
            })
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<Recipe> {
        self.list().into_iter().find(|r| r.name == name)
    }

    /// Saves `recipe`; `replacing` is the name it had when it was edited (a rename removes the old file).
    pub fn save(&self, recipe: &Recipe, replacing: Option<&str>) -> Result<()> {
        if recipe.name.trim().is_empty() {
            return Err(Error::Decode(tlmbench_decode::Error::Descriptor("a recipe needs a name".into())));
        }
        fs::create_dir_all(&self.dir).map_err(Error::io(&self.dir))?;
        if let Some(old) = replacing.filter(|o| *o != recipe.name) {
            self.delete(old)?;
        }
        // another recipe whose file name would be the same keeps its file: ours gets a number
        let mut path = self.dir.join(format!("{}.toml", slug(&recipe.name)));
        let mut n = 2;
        while path.exists() && !self.holds(&path, &recipe.name) {
            path = self.dir.join(format!("{}-{n}.toml", slug(&recipe.name)));
            n += 1;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, tlmbench_expr::recipe::to_toml(std::slice::from_ref(recipe))).map_err(Error::io(&tmp))?;
        fs::rename(&tmp, &path).map_err(Error::io(&path))
    }

    fn holds(&self, path: &Path, name: &str) -> bool {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| tlmbench_expr::recipe::from_toml(&t).ok())
            .is_some_and(|v| v.iter().any(|r| r.name == name))
    }

    /// Removes the recipe called `name` (from its file, which goes with it when it holds no other).
    pub fn delete(&self, name: &str) -> Result<()> {
        for f in self.files() {
            let Ok(Ok(mut v)) = fs::read_to_string(&f).map(|t| tlmbench_expr::recipe::from_toml(&t)) else { continue };
            if v.iter().any(|r| r.name == name) {
                v.retain(|r| r.name != name);
                if v.is_empty() {
                    fs::remove_file(&f).map_err(Error::io(&f))?;
                } else {
                    fs::write(&f, tlmbench_expr::recipe::to_toml(&v)).map_err(Error::io(&f))?;
                }
            }
        }
        Ok(())
    }

    /// Adds the recipes in a recipe file's text (a recipe of the same name is replaced); returns their names.
    pub fn import(&self, text: &str) -> std::result::Result<Vec<String>, String> {
        let recipes = tlmbench_expr::recipe::from_toml(text)?;
        for r in &recipes {
            self.save(r, None).map_err(|e| e.to_string())?;
        }
        Ok(recipes.into_iter().map(|r| r.name).collect())
    }

    /// The named recipes as a recipe file's text (all of them when `names` is empty).
    pub fn export(&self, names: &[String]) -> String {
        let all = self.list();
        let picked: Vec<Recipe> = all.into_iter().filter(|r| names.is_empty() || names.contains(&r.name)).collect();
        tlmbench_expr::recipe::to_toml(&picked)
    }
}

/// What the Derive dialog shows under the expression.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Preview {
    /// Samples the preview covers (the start of a long recording).
    pub samples: usize,
    /// Of those, the ones that hold a value.
    pub values: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    /// Set when the expression is one number (a flat line).
    pub number: Option<f64>,
}

/// The most samples a preview reads.
pub const PREVIEW_SAMPLES: usize = 1 << 20;

/// An environment holding only the first `n` samples of a recording.
struct Head<'a> {
    inner: RecEnv<'a>,
    n: usize,
}

impl Env for Head<'_> {
    fn series(&self, name: &str) -> Option<Series> {
        self.read(name, 0..self.n)
    }
    fn read(&self, name: &str, r: std::ops::Range<usize>) -> Option<Series> {
        self.inner.read(name, r.start.min(self.n)..r.end.min(self.n))
    }
    fn samples(&self) -> Option<usize> {
        Some(self.n)
    }
    fn hz(&self) -> Option<f64> {
        self.inner.hz()
    }
    fn label(&self, c: &str, t: &str) -> Option<f64> {
        self.inner.label(c, t)
    }
    fn limits(&self, c: &str, alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        self.inner.limits(c, alarm)
    }
}

/// Checks `expr` against `rec`'s channels and summarizes what it gives over the first [`PREVIEW_SAMPLES`] samples.
/// Errors name the character they point at.
pub fn check(rec: &dyn Recording, expr: &str) -> std::result::Result<Preview, ExprError> {
    check_cross(rec, expr, &[])
}

/// [`check`] where the expression may also read channels of the tab's other files.
pub fn check_cross(
    rec: &dyn Recording,
    expr: &str,
    foreign: &[crate::Foreign],
) -> std::result::Result<Preview, ExprError> {
    let tree = tlmbench_expr::parse(expr)?;
    let n = rec.info().samples.min(PREVIEW_SAMPLES);
    let env = Head { inner: RecEnv::new(rec).with_foreign(foreign), n };
    let (mut values, mut min, mut max, mut sum) = (0usize, f64::INFINITY, f64::NEG_INFINITY, 0.0);
    let number = tlmbench_expr::run(&tree, &env, tlmbench_expr::CHUNK, &mut |_, s| {
        for &x in s.v.iter().filter(|x| !x.is_nan()) {
            values += 1;
            (min, max, sum) = (min.min(x), max.max(x), sum + x);
        }
    })?;
    Ok(match number {
        Some(x) => Preview { samples: n, values: n, min: Some(x), max: Some(x), mean: Some(x), number: Some(x) },
        None if values == 0 => Preview { samples: n, values: 0, min: None, max: None, mean: None, number: None },
        None => Preview {
            samples: n,
            values,
            min: Some(min),
            max: Some(max),
            mean: Some(sum / values as f64),
            number: None,
        },
    })
}
