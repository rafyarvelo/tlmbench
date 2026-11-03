//! The CSV writer: a header row (`name [unit]`, or units in a second row), then a row per sample. Missing values
//! (NaN) are empty fields.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use tlmbench_core::datetime::format_utc;
use tlmbench_core::{Column, Timestamp};

use crate::{Block, Layout, Options, Plan, Result, Sink, TimeFormat, io_err};

pub(crate) struct CsvSink<'a> {
    out: BufWriter<File>,
    path: &'a Path,
    plan: &'a Plan,
    delim: u8,
    line: String,
}

/// `s` as a CSV field: quoted when it holds the delimiter, a quote or a line break.
pub(crate) fn quote(s: &str, delim: u8) -> String {
    if s.bytes().any(|b| b == delim || b == b'"' || b == b'\n' || b == b'\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// The time column's name and how its values are written.
fn time_header(plan: &Plan) -> Option<&'static str> {
    Some(match plan.time? {
        TimeFormat::Ns => "time_ns",
        _ => "time",
    })
}

impl<'a> CsvSink<'a> {
    pub(crate) fn create(path: &'a Path, plan: &'a Plan, opts: &Options) -> Result<Self> {
        let mut out = BufWriter::with_capacity(1 << 20, File::create(path).map_err(io_err(path))?);
        let d = opts.delimiter;
        let sep = (d as char).to_string();
        let mut head = String::new();
        if let Some(note) = &plan.note {
            head.push_str(&format!("# {note}\n"));
        }
        let mut names: Vec<String> = time_header(plan).map(str::to_owned).into_iter().collect();
        let mut units: Vec<String> = time_header(plan).map(|_| String::new()).into_iter().collect();
        match plan.layout {
            Layout::Wide => {
                for c in &plan.channels {
                    match (&c.unit, opts.units_row) {
                        (Some(u), false) => names.push(quote(&format!("{} [{u}]", c.name), d)),
                        (u, _) => {
                            names.push(quote(&c.name, d));
                            units.push(quote(u.as_deref().unwrap_or(""), d));
                        }
                    }
                }
            }
            Layout::Long => {
                names.extend(["channel".to_owned(), "value".to_owned()]);
                units.extend([String::new(), String::new()]);
            }
        }
        head.push_str(&names.join(&sep));
        head.push('\n');
        if opts.units_row && plan.layout == Layout::Wide {
            head.push_str(&units.join(&sep));
            head.push('\n');
        }
        out.write_all(head.as_bytes()).map_err(io_err(path))?;
        Ok(Self { out, path, plan, delim: d, line: String::with_capacity(256) })
    }

    fn time(&self, t: Timestamp) -> String {
        match self.plan.time.unwrap_or_default() {
            TimeFormat::Ns => t.to_string(),
            TimeFormat::Iso => format_utc(t),
            TimeFormat::Seconds => seconds(t),
            TimeFormat::Auto => {
                if self.plan.absolute {
                    format_utc(t)
                } else {
                    seconds(t)
                }
            }
        }
    }
}

/// Nanoseconds as decimal seconds, exactly (trailing zeros dropped).
fn seconds(t: Timestamp) -> String {
    let (sign, a) = if t < 0 { ("-", t.unsigned_abs()) } else { ("", t as u64) };
    let (s, ns) = (a / 1_000_000_000, a % 1_000_000_000);
    if ns == 0 {
        format!("{sign}{s}")
    } else {
        let frac = format!("{ns:09}");
        format!("{sign}{s}.{}", frac.trim_end_matches('0'))
    }
}

/// Value `i` of `c` as text: shortest round-trip form; NaN is empty.
fn value(c: &Column, i: usize, out: &mut String) {
    use std::fmt::Write as _;
    let _ = match c {
        Column::I8(v) => write!(out, "{}", v[i]),
        Column::I16(v) => write!(out, "{}", v[i]),
        Column::I32(v) => write!(out, "{}", v[i]),
        Column::I64(v) => write!(out, "{}", v[i]),
        Column::U8(v) => write!(out, "{}", v[i]),
        Column::U16(v) => write!(out, "{}", v[i]),
        Column::U32(v) => write!(out, "{}", v[i]),
        Column::U64(v) => write!(out, "{}", v[i]),
        Column::F32(v) if v[i].is_nan() => Ok(()),
        Column::F32(v) => write!(out, "{}", v[i]),
        Column::F64(v) if v[i].is_nan() => Ok(()),
        Column::F64(v) => write!(out, "{}", v[i]),
    };
}

impl Sink for CsvSink<'_> {
    fn write(&mut self, block: &Block) -> Result<()> {
        let n = block.columns.first().map_or(0, Column::len);
        let d = self.delim as char;
        for i in 0..n {
            let t = block.times.as_ref().map(|t| self.time(t[i]));
            match self.plan.layout {
                Layout::Wide => {
                    self.line.clear();
                    if let Some(t) = &t {
                        self.line.push_str(t);
                    }
                    for (k, c) in block.columns.iter().enumerate() {
                        if k > 0 || t.is_some() {
                            self.line.push(d);
                        }
                        value(c, i, &mut self.line);
                    }
                    self.line.push('\n');
                    self.out.write_all(self.line.as_bytes()).map_err(io_err(self.path))?;
                }
                Layout::Long => {
                    for (k, c) in block.columns.iter().enumerate() {
                        self.line.clear();
                        if let Some(t) = &t {
                            self.line.push_str(t);
                            self.line.push(d);
                        }
                        self.line.push_str(&quote(&self.plan.channels[k].name, self.delim));
                        self.line.push(d);
                        let v = c.get_f64(i);
                        if !v.is_nan() {
                            self.line.push_str(&v.to_string());
                        }
                        self.line.push('\n');
                        self.out.write_all(self.line.as_bytes()).map_err(io_err(self.path))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.out.flush().map_err(io_err(self.path))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn seconds_are_exact() {
        assert_eq!(super::seconds(1_500_000_000), "1.5");
        assert_eq!(super::seconds(-1), "-0.000000001");
        assert_eq!(super::seconds(2_000_000_000), "2");
    }
}
