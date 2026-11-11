//! Time that goes backwards in a file: flagged in the index, never silently reordered. Shared by the CSV and Parquet
//! readers, and by the browser-only core, which includes this file as it is.

use serde::{Deserialize, Serialize};

/// Time that goes backwards in a file: flagged in the index, never silently reordered. The rows stay in file order and a
/// row that reads earlier than the ones before it takes their latest time (so the times stay searchable); each place is
/// kept here with the time the file said, and the person is told when the file opens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Backwards {
    /// How many rows read earlier than a row before them.
    pub count: usize,
    /// The first few: (data row, the time reached so far, the time the row says), nanoseconds.
    pub first: Vec<(usize, i64, i64)>,
}

impl Backwards {
    const KEPT: usize = 100;

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The note for the person: how many places, and the first.
    pub fn note(&self) -> Option<String> {
        let &(row, reached, said) = self.first.first()?;
        let s = |ns: i64| format!("{} s", ns as f64 / 1e9);
        Some(format!(
            "Time goes backwards in {} row{} (first at data row {row}: it says {}, after {}). The rows stay in file order and each takes the latest time before it, so they stack at one instant; the file isn't reordered.",
            self.count,
            if self.count == 1 { "" } else { "s" },
            s(said),
            s(reached)
        ))
    }
}

/// Makes `t` (the times of the rows from data row `first_row`, the header being row 1) non-decreasing from `prev`, the
/// latest time so far, recording each row that read earlier.
pub fn clamp_backwards(t: &mut [i64], prev: &mut Option<i64>, first_row: usize, found: &mut Backwards) {
    for (i, x) in t.iter_mut().enumerate() {
        match *prev {
            Some(p) if *x < p => {
                found.count += 1;
                if found.first.len() < Backwards::KEPT {
                    found.first.push((first_row + i, p, *x));
                }
                *x = p;
            }
            _ => *prev = Some(*x),
        }
    }
}
