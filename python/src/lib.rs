//! The Python bindings: the same core as the app and the command line (`tlmbench-store` opens recordings, the same
//! queries answer, the same writers convert), exposed to `tlmbench/__init__.py`. Bulk data crosses as little-endian
//! bytes the Python side wraps as NumPy arrays; structured answers cross as JSON.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use tlmbench_core::Viewport;
use tlmbench_store::{Opened, Precision, Store};

fn err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn json(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// A recording opened through the store: decoded once into the app's cache (the same cache the app and CLI use).
#[pyclass(module = "tlmbench._core")]
struct Rec {
    opened: Arc<Opened>,
    config: Vec<PathBuf>,
}

fn store_for(cache: Option<&str>, descriptor_dirs: &[PathBuf]) -> Store {
    tlmbench_decode::descriptor::set_config_dirs(descriptor_dirs.to_vec());
    Store::new(cache.map(Path::new))
}

#[pyfunction]
#[pyo3(signature = (path, descriptor=None, cache=None, config=None))]
fn open(path: &str, descriptor: Option<&str>, cache: Option<&str>, config: Option<Vec<String>>) -> PyResult<Rec> {
    let config: Vec<PathBuf> = match config {
        Some(c) => c.into_iter().map(PathBuf::from).collect(),
        None => tlmbench_store::config::nearest(Path::new(path)).into_iter().collect(),
    };
    let store = store_for(cache, &config);
    let opened = store.open(Path::new(path), descriptor.map(Path::new)).map_err(err)?;
    Ok(Rec { opened: Arc::new(opened), config })
}

fn bytes_i64(py: Python<'_>, v: &[i64]) -> Py<PyBytes> {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    PyBytes::new(py, &b).into()
}

fn bytes_f64(py: Python<'_>, v: &[f64]) -> Py<PyBytes> {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    PyBytes::new(py, &b).into()
}

#[pymethods]
impl Rec {
    /// What the recording holds, as JSON: path, format, samples, time source, first and last times, channels.
    fn info(&self) -> PyResult<String> {
        serde_json::to_string(self.opened.recording().info()).map_err(err)
    }

    /// The nearest `.tlmbench/` folders configuration is read from.
    fn config_dirs(&self) -> Vec<String> {
        self.config.iter().map(|p| p.display().to_string()).collect()
    }

    /// Samples `start..end` (indexes): `(times_ns, [values...])`, each little-endian bytes (i64, f64 shown values).
    fn read(
        &self,
        py: Python<'_>,
        channels: Vec<usize>,
        start: usize,
        end: usize,
    ) -> PyResult<(Py<PyBytes>, Vec<Py<PyBytes>>)> {
        let rec = self.opened.recording();
        let n = rec.info().samples;
        let (start, end) = (start.min(n), end.min(n).max(start.min(n)));
        if let Some(&c) = channels.iter().find(|&&c| c >= rec.info().channels.len()) {
            return Err(PyValueError::new_err(format!("no channel {c}")));
        }
        let times = rec.read_times(start, end);
        let cols = channels
            .iter()
            .map(|&c| {
                let (scale, offset) = rec.scale_offset(c);
                bytes_f64(py, &rec.read(c, start, end).to_f64(scale, offset))
            })
            .collect();
        Ok((bytes_i64(py, &times), cols))
    }

    /// Index of the first sample at or after `t` (ns).
    fn lower_bound(&self, t: i64) -> usize {
        self.opened.recording().lower_bound(t)
    }

    /// The channel reduced to about `width` pixels over `[t0, t1)` (M4: the extremes of each pixel column, so a spike
    /// stays): `(times_ns, values)` as bytes (i64, f32).
    fn view(
        &self,
        py: Python<'_>,
        channel: usize,
        t0: i64,
        t1: i64,
        width: u32,
    ) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
        if channel >= self.opened.recording().info().channels.len() {
            return Err(PyValueError::new_err(format!("no channel {channel}")));
        }
        if t1 <= t0 {
            return Err(PyValueError::new_err("t1 must be after t0"));
        }
        let vp = Viewport { t0, t1, width_px: width.clamp(1, 16_384), raw_max: 0 };
        let v = self.opened.view(channel, vp, tlmbench_core::gaps::DEFAULT_FACTOR, Precision::Exact);
        let y: Vec<u8> = v.trace.y.iter().flat_map(|x| x.to_le_bytes()).collect();
        Ok((bytes_i64(py, &v.trace.t), PyBytes::new(py, &y).into()))
    }

    /// The sample nearest `t` for each channel, as JSON `{index, t, values}` (null when empty).
    fn values_at(&self, channels: Vec<usize>, t: i64) -> PyResult<String> {
        let v = self.opened.values_at(&channels, t).map(|(i, ts, values)| {
            serde_json::json!({ "index": i, "t": ts, "values": values.iter().map(|v| (!v.is_nan()).then_some(*v)).collect::<Vec<_>>() })
        });
        Ok(json(&v.unwrap_or(serde_json::Value::Null)))
    }

    /// Exact count, min, max, mean and standard deviation of each channel over `[t0, t1)`, as JSON (null until the
    /// pyramid is built, which `build()` does).
    fn stats(&self, channels: Vec<usize>, t0: i64, t1: i64) -> PyResult<String> {
        let Some(all) = self.opened.stats(&channels, t0, t1) else { return Ok("null".into()) };
        let num = |v: f64| (!v.is_nan()).then_some(v);
        let out: Vec<_> = channels
            .iter()
            .zip(all)
            .map(|(&c, s)| {
                let at = |m: Option<(f64, usize)>| m.map(|(_, i)| self.opened.recording().time_at(i));
                serde_json::json!({
                    "channel": c, "count": s.count, "min": s.min.map(|m| m.0), "min_t": at(s.min),
                    "max": s.max.map(|m| m.0), "max_t": at(s.max), "mean": num(s.mean), "std": num(s.std),
                })
            })
            .collect();
        Ok(json(&serde_json::Value::Array(out)))
    }

    /// Builds the pyramid (exact stats and fast views need it); a no-op once built.
    fn build(&self, py: Python<'_>) -> PyResult<()> {
        let o = self.opened.clone();
        py.detach(move || o.build(&mut |_, _| {})).map_err(err)
    }

    /// Derived channels (design: Derived channels): `recipes` are `(name, expression, unit, category, decimals)` computed
    /// over this recording's channels; with none given, the recipes of the nearest `.tlmbench/` that apply. Returns a new
    /// recording holding this one's channels and then the derived ones, and the recipes that don't apply with why.
    #[pyo3(signature = (recipes=None))]
    #[allow(clippy::type_complexity)]
    fn derive(
        &self,
        py: Python<'_>,
        recipes: Option<Vec<(String, String, Option<String>, Option<String>, Option<u8>)>>,
    ) -> PyResult<(Rec, Vec<(String, String)>)> {
        let list: Vec<tlmbench_expr::Recipe> = match recipes {
            Some(r) => r
                .into_iter()
                .map(|(name, expr, unit, category, decimals)| tlmbench_expr::Recipe {
                    name,
                    expr,
                    unit,
                    category,
                    decimals,
                    automatic: false,
                })
                .collect(),
            None => tlmbench_store::config::shared_recipes(&self.config),
        };
        let base = self.opened.clone();
        let store = base.store().clone();
        let (joined, skipped) = py
            .detach(move || -> Result<_, tlmbench_store::Error> {
                let d = store.open_derived(&base, &list)?;
                let skipped: Vec<(String, String)> =
                    d.skipped.iter().map(|(n, e)| (n.clone(), e.msg.clone())).collect();
                let opened = match d.opened {
                    Some(o) => store.join(base.clone(), Arc::new(o))?,
                    None => return Ok((None, skipped)),
                };
                Ok((Some(opened), skipped))
            })
            .map_err(err)?;
        let opened = match joined {
            Some(o) => Arc::new(o),
            None => self.opened.clone(),
        };
        Ok((Rec { opened, config: self.config.clone() }, skipped))
    }

    /// The events kept for this recording, as JSON: the ones you added (in the app's cache) and the ones shared in the
    /// nearest `.tlmbench/events/`, in start order.
    fn events(&self) -> PyResult<String> {
        let o = &self.opened;
        let mut all = tlmbench_store::events::list(o.store().root(), o.identity()).map_err(err)?;
        let name = o.recording().info().path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for e in tlmbench_store::config::shared_events(&self.config, o.identity(), &name) {
            if !all.iter().any(|x| x.id == e.id) {
                all.push(e);
            }
        }
        all.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
        serde_json::to_string(&all).map_err(err)
    }

    /// Adds an event (a marker when `duration_ns` is 0, else a range) to this recording's own events in the app's cache:
    /// nothing is written beside the data. Returns the event as JSON.
    #[pyo3(signature = (name, start_ns, duration_ns=0, category="", channels=None))]
    fn add_event(
        &self,
        name: &str,
        start_ns: i64,
        duration_ns: i64,
        category: &str,
        channels: Option<Vec<String>>,
    ) -> PyResult<String> {
        if name.trim().is_empty() {
            return Err(PyValueError::new_err("an event needs a name"));
        }
        let o = &self.opened;
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let ev = tlmbench_store::events::Event {
            id: format!("py{at:x}"),
            name: name.trim().to_owned(),
            start: start_ns.to_string(),
            duration: duration_ns.to_string(),
            category: category.to_owned(),
            source: "user".into(),
            channels: channels.unwrap_or_default(),
            by: None,
        };
        tlmbench_store::events::save(o.store().root(), o.identity(), ev.clone()).map_err(err)?;
        serde_json::to_string(&ev).map_err(err)
    }

    /// Removes one of the events you added, by id.
    fn delete_event(&self, id: &str) -> PyResult<()> {
        let o = &self.opened;
        tlmbench_store::events::delete(o.store().root(), o.identity(), id).map_err(err)?;
        Ok(())
    }

    /// Runs a check set (TOML text, or the name of one in the nearest `.tlmbench/checks`): its results as JSON.
    fn check(&self, py: Python<'_>, set: &str) -> PyResult<String> {
        let set = if set.contains('\n') || set.contains('=') {
            tlmbench_store::checks::from_toml(set).map_err(PyValueError::new_err)?
        } else {
            let mut found = None;
            for d in self.config.iter().rev() {
                found = found.or_else(|| tlmbench_store::checks::Sets::in_dir(&d.join("checks")).get(set));
            }
            found.ok_or_else(|| PyValueError::new_err(format!("no check set called {set:?}")))?
        };
        let o = self.opened.clone();
        let store = o.store().clone();
        let recipes = tlmbench_store::config::shared_recipes(&self.config);
        let res = py.detach(move || store.run_checks(&o, &set, &recipes)).map_err(err)?;
        serde_json::to_string(&res).map_err(err)
    }

    /// Writes the recording (`fmt`: csv, parquet, arrow, hdf5 or mat) to `out`; the path written and the rows.
    #[pyo3(signature = (out, fmt, channels=None, start=0, end=None))]
    fn convert(
        &self,
        py: Python<'_>,
        out: &str,
        fmt: &str,
        channels: Option<Vec<usize>>,
        start: usize,
        end: Option<usize>,
    ) -> PyResult<(String, u64)> {
        use tlmbench_export::{Format, Options};
        let format = match fmt {
            "csv" => Format::Csv,
            "parquet" => Format::Parquet,
            "arrow" => Format::Arrow,
            "hdf5" => Format::Hdf5,
            "mat" => Format::Mat,
            other => {
                return Err(PyValueError::new_err(format!(
                    "{other:?} isn't a format (csv, parquet, arrow, hdf5, mat)"
                )));
            }
        };
        let o = self.opened.clone();
        let n = o.recording().info().samples;
        let chans = channels.unwrap_or_else(|| (0..o.recording().info().channels.len()).collect());
        let opts = Options::new(format, chans, start, end.unwrap_or(n).min(n));
        let out = PathBuf::from(out);
        let w = py.detach(move || tlmbench_export::export(o.recording(), &opts, &out)).map_err(err)?;
        Ok((w.path.display().to_string(), w.rows))
    }

    /// Export to HTML: the same folder as `tlmbench export html`. `layout` is a shared view as JSON text or empty.
    #[pyo3(signature = (out, layout=None, channels=None, raw=None, mode="auto", budget=None))]
    #[allow(clippy::too_many_arguments)]
    fn export_html(
        &self,
        py: Python<'_>,
        out: &str,
        layout: Option<&str>,
        channels: Option<Vec<usize>>,
        raw: Option<Vec<usize>>,
        mode: &str,
        budget: Option<u64>,
    ) -> PyResult<String> {
        use tlmbench_export::html;
        let layout: serde_json::Value = match layout {
            Some(t) if !t.trim().is_empty() => {
                serde_json::from_str(t).map_err(|e| PyValueError::new_err(format!("the layout: {e}")))?
            }
            _ => serde_json::Value::Null,
        };
        let mode = match mode {
            "full" => html::Mode::Full,
            "light" => html::Mode::Light,
            "auto" => html::Mode::Auto,
            other => return Err(PyValueError::new_err(format!("{other:?} isn't a mode (auto, full, light)"))),
        };
        let o = self.opened.clone();
        let info = o.recording().info().clone();
        let chans = channels.unwrap_or_else(|| (0..info.channels.len()).collect());
        let out = PathBuf::from(out);
        let report = py
            .detach(move || {
                let f = html::File { name: info.path.file_name().map_or_else(|| "recording".into(), |n| n.to_string_lossy().into_owned()), rec: o.recording(), channels: chans, raw: raw.unwrap_or_default(), windows: Vec::new() };
                let extra = serde_json::json!({ "layout": layout, "events": [], "checks": [], "tool": { "name": "tlmbench-python" } });
                let opts = html::Options { mode, budget: budget.unwrap_or(html::DEFAULT_BUDGET), from: None, until: None };
                let report = html::export(&out, &[f], extra, &opts)?;
                let built_in = html::embedded_viewer();
                if !built_in.is_empty() {
                    html::write_viewer(&out, &built_in)?;
                }
                Ok::<_, tlmbench_export::Error>(report)
            })
            .map_err(err)?;
        serde_json::to_string(&report).map_err(err)
    }
}

/// Every object in a `.tlmbench/` folder (or the nearest one above `path`), as JSON: `{"dir", "objects": [{"kind", "name",
/// "files": {relative path: text}}], "problems": [{"path", "why"}]}`. A folder with none has nothing.
#[pyfunction]
fn config_objects(path: &str) -> PyResult<String> {
    let p = Path::new(path);
    let dir = if p.file_name().is_some_and(|n| n == ".tlmbench") {
        Some(p.to_path_buf())
    } else {
        tlmbench_store::config::nearest(p)
    };
    let Some(dir) = dir else { return Ok(json(&serde_json::json!({ "dir": null, "objects": [], "problems": [] }))) };
    let (items, problems) = tlmbench_store::config::read_folder(&dir);
    let objects: Vec<serde_json::Value> = items
        .iter()
        .map(|i| {
            let files: serde_json::Map<String, serde_json::Value> = i
                .files
                .iter()
                .map(|(p, b)| (p.clone(), serde_json::Value::String(String::from_utf8_lossy(b).into_owned())))
                .collect();
            serde_json::json!({ "kind": i.kind, "name": i.name, "files": files })
        })
        .collect();
    let problems: Vec<serde_json::Value> =
        problems.iter().map(|x| serde_json::json!({ "path": x.path, "why": x.why })).collect();
    Ok(json(&serde_json::json!({ "dir": dir.display().to_string(), "objects": objects, "problems": problems })))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_function(wrap_pyfunction!(config_objects, m)?)?;
    m.add_class::<Rec>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
