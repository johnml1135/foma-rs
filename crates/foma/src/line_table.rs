//! The FSM line table — the arc storage of an [`Fsm`](crate::types::Fsm).
//!
//! Historically this was a bare, sentinel-terminated `Vec<FsmState>`: one row
//! per arc, rows grouped by `state_no`, the whole run closed by a `state_no ==
//! -1` terminator row. A state with no outgoing arcs still occupies one
//! "marker" row (`target == -1`) that records its `final_state`/`start_state`.
//! Every consumer walked that flat table by index, peeking `fsm[i+1].state_no`
//! to find state boundaries.
//!
//! `LineTable` is the seam that let the backing store change without rewriting
//! all those walks in one commit. It now stores the compressed [`Csr`] form
//! (each arc drops its redundant `state_no`/`final_state`/`start_state`, which
//! are properties of the state, not the arc), roughly halving arc memory.
//! Consumers that still think in flat rows borrow a materialized view through
//! [`LineTable::rows`] / [`LineTable::rows_mut`]; the mutable guard recompresses
//! on drop. Hot paths read the [`Csr`] blocks directly.

use core::ops::{Deref, DerefMut};

use crate::types::FsmState;

/// The line table of an [`Fsm`](crate::types::Fsm), stored compressed.
///
/// An empty table (C `NULL` `net->states`) is [`LineTable::new`].
#[derive(Debug, Clone, Default)]
pub struct LineTable {
    csr: Csr,
}

impl LineTable {
    /// An empty table (C: a `NULL` `net->states`).
    pub fn new() -> LineTable {
        LineTable {
            csr: Csr::default(),
        }
    }

    /// Compress a flat, sentinel-terminated row sequence into a table.
    pub fn from_rows(rows: Vec<FsmState>) -> LineTable {
        LineTable {
            csr: Csr::from_rows(&rows),
        }
    }

    /// Consume the table, materializing the flat row sequence.
    pub fn into_rows(self) -> Vec<FsmState> {
        self.csr.to_rows()
    }

    /// A materialized, read-only view of the flat rows. Derefs to
    /// `[FsmState]`, so existing `fsm[i]` / `fsm[i+1]` walks read unchanged.
    pub fn rows(&self) -> RowsRef {
        RowsRef {
            rows: self.csr.to_rows(),
        }
    }

    /// A materialized, mutable view of the flat rows that recompresses into the
    /// table when the guard drops. Derefs to `Vec<FsmState>` (push/truncate/
    /// splice and in-place field writes all work).
    pub fn rows_mut(&mut self) -> RowsMut<'_> {
        let rows = self.csr.to_rows();
        RowsMut { table: self, rows }
    }

    /// The compressed blocks, in appearance order (native, no materialization).
    pub fn blocks(&self) -> &[StateBlock] {
        self.csr.blocks()
    }

    /// The arcs of one block (native, no materialization).
    pub fn block_arcs(&self, b: &StateBlock) -> &[CsrArc] {
        self.csr.block_arcs(b)
    }

    /// Logical row count including the terminator — the C `linecount`.
    pub fn len(&self) -> usize {
        self.csr.logical_len()
    }

    /// True for the empty (C `NULL`) table.
    pub fn is_empty(&self) -> bool {
        self.csr.is_null()
    }
}

impl From<Vec<FsmState>> for LineTable {
    fn from(rows: Vec<FsmState>) -> LineTable {
        LineTable {
            csr: Csr::from_rows(&rows),
        }
    }
}

/// Read guard: a materialized copy of the flat rows.
pub struct RowsRef {
    rows: Vec<FsmState>,
}

impl Deref for RowsRef {
    type Target = [FsmState];
    fn deref(&self) -> &[FsmState] {
        &self.rows
    }
}

/// Mutable guard: a materialized copy that recompresses into the source table
/// when dropped.
pub struct RowsMut<'a> {
    table: &'a mut LineTable,
    rows: Vec<FsmState>,
}

impl Deref for RowsMut<'_> {
    type Target = Vec<FsmState>;
    fn deref(&self) -> &Vec<FsmState> {
        &self.rows
    }
}

impl DerefMut for RowsMut<'_> {
    fn deref_mut(&mut self) -> &mut Vec<FsmState> {
        &mut self.rows
    }
}

impl Drop for RowsMut<'_> {
    fn drop(&mut self) {
        self.table.csr = Csr::from_rows(&self.rows);
    }
}

/* ------------------------------------------------------------------ */
/* Compressed backing store (adopted by LineTable in a later stage)   */
/* ------------------------------------------------------------------ */

/// One contiguous run of same-numbered rows in the flat table, compressed.
///
/// `state_no`, `final_state` and `start_state` are constant across a state's
/// rows in every foma-produced table (`fsm_count` already reads final/start
/// only from a state's first row, and the builders write them uniformly), so
/// they are stored once here instead of once per arc. `arc_len == 0` marks a
/// state with no outgoing arcs — the flat table's single `target == -1` marker
/// row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateBlock {
    pub state_no: i32,
    pub final_state: i8,
    pub start_state: i8,
    /// First arc index in [`Csr::arcs`].
    pub arc_start: u32,
    /// Number of arcs (0 ↔ a marker/arc-less state).
    pub arc_len: u32,
}

/// The only per-arc data the flat row genuinely varies: label and target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsrArc {
    pub r#in: i16,
    pub out: i16,
    pub target: i32,
}

/// Compressed-sparse-row form of the line table: per-state blocks plus a flat
/// arc array grouped by block. Regenerates the exact flat rows (marker rows,
/// terminator) via [`Csr::to_rows`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Csr {
    blocks: Vec<StateBlock>,
    arcs: Vec<CsrArc>,
    /// The exact terminator row, captured verbatim so it round-trips byte-for-
    /// byte. `None` ↔ an empty (C `NULL`) table.
    terminator: Option<FsmState>,
}

impl Csr {
    /// Compress a flat, sentinel-terminated row sequence.
    pub fn from_rows(rows: &[FsmState]) -> Csr {
        if rows.is_empty() {
            return Csr::default();
        }
        let term_idx = rows
            .iter()
            .position(|r| r.state_no == -1)
            .expect("line table must be sentinel-terminated");
        let body = &rows[..term_idx];

        let mut blocks: Vec<StateBlock> = Vec::new();
        let mut arcs: Vec<CsrArc> = Vec::new();
        let mut i = 0usize;
        while i < body.len() {
            let sno = body[i].state_no;
            let fin = body[i].final_state;
            let start = body[i].start_state;
            let arc_start = arcs.len() as u32;
            let mut j = i;
            while j < body.len() && body[j].state_no == sno {
                let r = &body[j];
                debug_assert_eq!(
                    r.final_state, fin,
                    "final_state constant within a state run"
                );
                debug_assert_eq!(
                    r.start_state, start,
                    "start_state constant within a state run"
                );
                if r.target != -1 {
                    arcs.push(CsrArc {
                        r#in: r.r#in,
                        out: r.out,
                        target: r.target,
                    });
                } else {
                    debug_assert_eq!(r.r#in, -1, "marker row in == -1");
                    debug_assert_eq!(r.out, -1, "marker row out == -1");
                }
                j += 1;
            }
            blocks.push(StateBlock {
                state_no: sno,
                final_state: fin,
                start_state: start,
                arc_start,
                arc_len: arcs.len() as u32 - arc_start,
            });
            i = j;
        }
        Csr {
            blocks,
            arcs,
            terminator: Some(rows[term_idx]),
        }
    }

    /// Regenerate the flat, sentinel-terminated row sequence.
    pub fn to_rows(&self) -> Vec<FsmState> {
        let Some(term) = self.terminator else {
            return Vec::new();
        };
        let mut rows = Vec::with_capacity(self.arcs.len() + self.blocks.len() + 1);
        for b in &self.blocks {
            if b.arc_len == 0 {
                rows.push(FsmState {
                    state_no: b.state_no,
                    r#in: -1,
                    out: -1,
                    target: -1,
                    final_state: b.final_state,
                    start_state: b.start_state,
                });
            } else {
                let lo = b.arc_start as usize;
                let hi = lo + b.arc_len as usize;
                for a in &self.arcs[lo..hi] {
                    rows.push(FsmState {
                        state_no: b.state_no,
                        r#in: a.r#in,
                        out: a.out,
                        target: a.target,
                        final_state: b.final_state,
                        start_state: b.start_state,
                    });
                }
            }
        }
        rows.push(term);
        rows
    }

    /// State blocks, in appearance order.
    pub fn blocks(&self) -> &[StateBlock] {
        &self.blocks
    }

    /// The arcs of one block, in order.
    pub fn block_arcs(&self, b: &StateBlock) -> &[CsrArc] {
        let lo = b.arc_start as usize;
        &self.arcs[lo..lo + b.arc_len as usize]
    }

    /// Logical flat-row count including the terminator (the C `linecount`): one
    /// row per arc, one marker row per arc-less state, plus the terminator.
    /// Zero for the empty (C `NULL`) table.
    pub fn logical_len(&self) -> usize {
        if self.terminator.is_none() {
            return 0;
        }
        let body: usize = self
            .blocks
            .iter()
            .map(|b| (b.arc_len as usize).max(1))
            .sum();
        body + 1
    }

    /// True for the empty (C `NULL`) table — no terminator, no rows.
    pub fn is_null(&self) -> bool {
        self.terminator.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state_no: i32, r#in: i16, out: i16, target: i32, fin: i8, start: i8) -> FsmState {
        FsmState {
            state_no,
            r#in,
            out,
            target,
            final_state: fin,
            start_state: start,
        }
    }

    fn term() -> FsmState {
        row(-1, -1, -1, -1, -1, -1)
    }

    /// to_rows(from_rows(v)) == v for every valid flat table.
    fn assert_roundtrip(rows: &[FsmState]) {
        let csr = Csr::from_rows(rows);
        assert_eq!(csr.to_rows().as_slice(), rows);
    }

    #[test]
    fn empty_table_is_null() {
        let csr = Csr::from_rows(&[]);
        assert!(csr.to_rows().is_empty());
        assert!(csr.blocks().is_empty());
    }

    #[test]
    fn fsm_empty_set_shape() {
        // state 0: arc-less, non-final, start; then terminator (fsm_empty()).
        let rows = [row(0, -1, -1, -1, 0, 1), term()];
        assert_roundtrip(&rows);
        let csr = Csr::from_rows(&rows);
        assert_eq!(csr.blocks().len(), 1);
        assert_eq!(csr.blocks()[0].arc_len, 0);
        assert_eq!(csr.blocks()[0].start_state, 1);
    }

    #[test]
    fn fsm_identity_shape() {
        // 0 -id->2 (start), 1 final marker, 2 terminator-adjacent marker.
        let rows = [row(0, 2, 2, 1, 0, 1), row(1, -1, -1, -1, 1, 0), term()];
        assert_roundtrip(&rows);
        let csr = Csr::from_rows(&rows);
        assert_eq!(csr.blocks().len(), 2);
        assert_eq!(csr.block_arcs(&csr.blocks()[0]).len(), 1);
        assert_eq!(csr.block_arcs(&csr.blocks()[0])[0].target, 1);
        assert!(csr.block_arcs(&csr.blocks()[1]).is_empty());
        assert_eq!(csr.blocks()[1].final_state, 1);
    }

    #[test]
    fn multi_arc_state_final_start_constant() {
        // state 0 (start, non-final) with three arcs; state 1 final, arc-less.
        let rows = [
            row(0, 3, 3, 1, 0, 1),
            row(0, 4, 4, 1, 0, 1),
            row(0, 5, 5, 1, 0, 1),
            row(1, -1, -1, -1, 1, 0),
            term(),
        ];
        assert_roundtrip(&rows);
        let csr = Csr::from_rows(&rows);
        assert_eq!(csr.blocks().len(), 2);
        assert_eq!(csr.blocks()[0].arc_len, 3);
        // arc labels preserved in order.
        let labels: Vec<i16> = csr
            .block_arcs(&csr.blocks()[0])
            .iter()
            .map(|a| a.r#in)
            .collect();
        assert_eq!(labels, vec![3, 4, 5]);
    }

    #[test]
    fn non_canonical_terminator_preserved() {
        // Only state_no == -1 terminates the walk; other fields are captured
        // verbatim so an oddly-shaped terminator still round-trips.
        let rows = [row(0, 7, 8, 0, 1, 1), row(-1, 9, 9, 9, 0, 0)];
        assert_roundtrip(&rows);
    }

    #[test]
    fn repeated_state_number_runs_kept_separate() {
        // A pathological table where state 0 reappears after state 1: each
        // contiguous run is its own block (matches the flat-walk's grouping).
        let rows = [
            row(0, 3, 3, 1, 0, 1),
            row(1, 4, 4, 0, 0, 0),
            row(0, 5, 5, 1, 0, 1),
            term(),
        ];
        assert_roundtrip(&rows);
        assert_eq!(Csr::from_rows(&rows).blocks().len(), 3);
    }

    #[test]
    fn generated_tables_roundtrip() {
        // Deterministic pseudo-random valid tables: ascending dense states,
        // each with 0..=3 arcs, constant final/start per state.
        let mut seed = 0x1234_5678u32;
        let mut rng = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) & 0x7fff
        };
        for _ in 0..200 {
            let nstates = 1 + (rng() % 8) as i32;
            let mut rows = Vec::new();
            for s in 0..nstates {
                let fin = (rng() % 2) as i8;
                let start = if s == 0 { 1 } else { 0 };
                let narcs = rng() % 4;
                if narcs == 0 {
                    rows.push(row(s, -1, -1, -1, fin, start));
                } else {
                    for _ in 0..narcs {
                        let sym = 3 + (rng() % 6) as i16;
                        let tgt = (rng() % nstates as u32) as i32;
                        rows.push(row(s, sym, sym, tgt, fin, start));
                    }
                }
            }
            rows.push(term());
            assert_roundtrip(&rows);
        }
    }
}
