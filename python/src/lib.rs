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
                return Err(PyValueError::new_err(format!("{other:?} isn't a format (csv, parquet, arrow, hdf5, mat)")));
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
                let f = html::File { name: info.path.file_name().map_or_else(|| "recording".into(), |n| n.to_string_lossy().into_owned()), rec: o.recording(), channels: chans, raw: raw.unwrap_or_default() };
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

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_class::<Rec>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
