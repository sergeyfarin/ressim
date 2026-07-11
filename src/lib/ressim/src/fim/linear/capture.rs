//! Native-only capture of failing FIM linear systems for offline analysis.
//!
//! Phase 9 (component-isolation lab): every prior linear-solver hypothesis had to be
//! tested by changing the live solver and replaying full simulations, which conflates
//! linear-solve quality with Newton-trajectory and timestep-controller feedback. This
//! module lets the Newton loop dump the exact `jacobian`/`rhs`/`layout` of a failed
//! iterative solve to a plain-text file (std-only, no new dependencies), so the offline
//! solver lab (`solver_lab.rs`) can re-run full solves with alternative
//! preconditioner/restriction variants on identical real inputs.
//!
//! Capture is off unless the `FIM_CAPTURE_DIR` environment variable is set, and the
//! whole module is compiled out of wasm builds — production behavior is untouched.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use nalgebra::DVector;
use sprs::CsMat;
#[cfg(test)]
use sprs::TriMatI;

use super::FimLinearBlockLayout;
use crate::fim::scaling::EquationScaling;

pub(crate) const CAPTURE_DIR_ENV: &str = "FIM_CAPTURE_DIR";

/// Bundle P (`FIM-BUNDLE-P`) P0.2: a second, distinct capture directory that dumps *every*
/// linear system actually solved (converged or not), not just failures/near-misses. The
/// offline CPR-setup-reuse staleness study needs truly consecutive Newton-iteration systems
/// to measure "build on system i, reuse across i+1..i+k" — the failure-only corpus above
/// captures isolated snapshots (usually one every few dozen substeps), not a sequence.
pub(crate) const CAPTURE_SEQUENCE_DIR_ENV: &str = "FIM_CAPTURE_SEQUENCE_DIR";

/// Process-wide monotonically increasing capture sequence. `run_fim_timestep` is called
/// once per substep/retry rung, so a per-call counter would overwrite earlier files —
/// this gives every captured system in a run a unique filename.
static CAPTURE_SEQUENCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(crate) fn next_capture_sequence() -> usize {
    CAPTURE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Metadata identifying where in the run a captured system came from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimCaptureMetadata {
    pub(crate) newton_iteration: usize,
    pub(crate) failure_reason: String,
    pub(crate) dominant_family: String,
    pub(crate) dominant_item_index: usize,
}

/// A captured linear system, as loaded back from disk. Loading is only exercised by the
/// offline solver lab, which is test-only.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct FimCapturedSystem {
    pub(crate) metadata: FimCaptureMetadata,
    pub(crate) layout: Option<FimLinearBlockLayout>,
    pub(crate) jacobian: CsMat<f64>,
    pub(crate) rhs: DVector<f64>,
    pub(crate) equation_scaling: Option<EquationScaling>,
}

/// Returns the capture directory if capture is enabled via `FIM_CAPTURE_DIR`.
pub(crate) fn capture_dir_from_env() -> Option<PathBuf> {
    std::env::var_os(CAPTURE_DIR_ENV).map(PathBuf::from)
}

/// Returns the sequential-capture directory if enabled via `FIM_CAPTURE_SEQUENCE_DIR`.
pub(crate) fn capture_sequence_dir_from_env() -> Option<PathBuf> {
    std::env::var_os(CAPTURE_SEQUENCE_DIR_ENV).map(PathBuf::from)
}

/// Writes one failed system to `<dir>/fim_capture_<seq>.txt`. Errors are reported to
/// stderr and swallowed: capture is a diagnostic aid and must never abort a solve.
pub(crate) fn write_capture(
    dir: &Path,
    sequence: usize,
    metadata: &FimCaptureMetadata,
    layout: Option<FimLinearBlockLayout>,
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    equation_scaling: Option<&EquationScaling>,
) {
    if let Err(error) = try_write_capture(
        dir,
        sequence,
        metadata,
        layout,
        jacobian,
        rhs,
        equation_scaling,
    ) {
        eprintln!("FIM capture: failed to write system {sequence}: {error}");
    }
}

fn try_write_capture(
    dir: &Path,
    sequence: usize,
    metadata: &FimCaptureMetadata,
    layout: Option<FimLinearBlockLayout>,
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    equation_scaling: Option<&EquationScaling>,
) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("fim_capture_{sequence:05}.txt"));
    let mut out = std::io::BufWriter::new(fs::File::create(path)?);

    writeln!(out, "fim-capture-v2")?;
    writeln!(out, "newton_iteration {}", metadata.newton_iteration)?;
    writeln!(out, "failure_reason {}", metadata.failure_reason)?;
    writeln!(out, "dominant_family {}", metadata.dominant_family)?;
    writeln!(out, "dominant_item_index {}", metadata.dominant_item_index)?;
    match layout {
        Some(layout) => writeln!(
            out,
            "layout {} {} {} {}",
            layout.cell_block_count,
            layout.cell_block_size,
            layout.well_bhp_count,
            layout.perforation_tail_start,
        )?,
        None => writeln!(out, "layout none")?,
    }
    fn write_scale_row(
        out: &mut impl std::io::Write,
        name: &str,
        values: &[f64],
    ) -> std::io::Result<()> {
        write!(out, "{name} {}", values.len())?;
        for value in values {
            write!(out, " {value:e}")?;
        }
        writeln!(out)
    }
    match equation_scaling {
        Some(scaling) => {
            writeln!(out, "equation_scaling 1")?;
            write_scale_row(&mut out, "water", &scaling.water)?;
            write_scale_row(&mut out, "oil_component", &scaling.oil_component)?;
            write_scale_row(&mut out, "gas_component", &scaling.gas_component)?;
            write_scale_row(&mut out, "well_constraint", &scaling.well_constraint)?;
            write_scale_row(&mut out, "perforation_flow", &scaling.perforation_flow)?;
        }
        None => writeln!(out, "equation_scaling 0")?,
    }
    writeln!(out, "rows {}", jacobian.rows())?;
    writeln!(out, "cols {}", jacobian.cols())?;
    writeln!(out, "nnz {}", jacobian.nnz())?;

    writeln!(out, "rhs")?;
    for value in rhs.iter() {
        writeln!(out, "{value:e}")?;
    }

    writeln!(out, "triplets")?;
    for (row, vec) in jacobian.outer_iterator().enumerate() {
        for (&col, &value) in vec.indices().iter().zip(vec.data().iter()) {
            writeln!(out, "{row} {col} {value:e}")?;
        }
    }

    Ok(())
}

/// Loads every `fim_capture_*.txt` in `dir`, sorted by filename.
#[cfg(test)]
pub(crate) fn load_captures(dir: &Path) -> std::io::Result<Vec<FimCapturedSystem>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("fim_capture_") && name.ends_with(".txt"))
        })
        .collect();
    paths.sort();

    paths.iter().map(|path| load_capture(path)).collect()
}

#[cfg(test)]
pub(crate) fn load_capture(path: &Path) -> std::io::Result<FimCapturedSystem> {
    let content = fs::read_to_string(path)?;
    parse_capture(&content).map_err(|message| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {message}", path.display()),
        )
    })
}

#[cfg(test)]
fn parse_capture(content: &str) -> Result<FimCapturedSystem, String> {
    let mut lines = content.lines();

    let header = lines.next().ok_or("empty file")?;
    if header != "fim-capture-v2" {
        return Err(format!("unexpected header {header:?}"));
    }

    fn field<'a>(line: Option<&'a str>, key: &str) -> Result<&'a str, String> {
        let line = line.ok_or_else(|| format!("missing {key}"))?;
        line.strip_prefix(key)
            .map(str::trim)
            .ok_or_else(|| format!("expected {key}, got {line:?}"))
    }

    let newton_iteration = field(lines.next(), "newton_iteration")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let failure_reason = field(lines.next(), "failure_reason")?.to_string();
    let dominant_family = field(lines.next(), "dominant_family")?.to_string();
    let dominant_item_index = field(lines.next(), "dominant_item_index")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;

    let layout_line = field(lines.next(), "layout")?;
    let layout = if layout_line == "none" {
        None
    } else {
        let parts: Vec<usize> = layout_line
            .split_whitespace()
            .map(|part| part.parse::<usize>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        if parts.len() != 4 {
            return Err(format!("layout expects 4 fields, got {}", parts.len()));
        }
        Some(FimLinearBlockLayout {
            cell_block_count: parts[0],
            cell_block_size: parts[1],
            well_bhp_count: parts[2],
            perforation_tail_start: parts[3],
        })
    };

    fn parse_scale_row(line: Option<&str>, key: &str) -> Result<Vec<f64>, String> {
        let rest = field(line, key)?;
        let mut parts = rest.split_whitespace();
        let count = parts
            .next()
            .ok_or_else(|| format!("{key} missing count"))?
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let values: Vec<f64> = parts
            .map(|part| part.parse::<f64>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        if values.len() != count {
            return Err(format!(
                "{key} expects {count} values, got {}",
                values.len()
            ));
        }
        Ok(values)
    }

    let equation_scaling_flag = field(lines.next(), "equation_scaling")?;
    let equation_scaling = match equation_scaling_flag {
        "0" => None,
        "1" => Some(EquationScaling {
            water: parse_scale_row(lines.next(), "water")?,
            oil_component: parse_scale_row(lines.next(), "oil_component")?,
            gas_component: parse_scale_row(lines.next(), "gas_component")?,
            well_constraint: parse_scale_row(lines.next(), "well_constraint")?,
            perforation_flow: parse_scale_row(lines.next(), "perforation_flow")?,
        }),
        other => return Err(format!("unexpected equation_scaling flag {other:?}")),
    };

    let rows = field(lines.next(), "rows")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let cols = field(lines.next(), "cols")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let nnz = field(lines.next(), "nnz")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;

    if lines.next() != Some("rhs") {
        return Err("expected rhs section".to_string());
    }
    let mut rhs = DVector::zeros(rows);
    for row in 0..rows {
        let line = lines.next().ok_or("rhs truncated")?;
        rhs[row] = line.trim().parse::<f64>().map_err(|e| e.to_string())?;
    }

    if lines.next() != Some("triplets") {
        return Err("expected triplets section".to_string());
    }
    let mut tri = TriMatI::<f64, usize>::new((rows, cols));
    let mut seen = 0usize;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let row = parts
            .next()
            .ok_or("triplet missing row")?
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let col = parts
            .next()
            .ok_or("triplet missing col")?
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let value = parts
            .next()
            .ok_or("triplet missing value")?
            .parse::<f64>()
            .map_err(|e| e.to_string())?;
        tri.add_triplet(row, col, value);
        seen += 1;
    }
    if seen != nnz {
        return Err(format!("expected {nnz} triplets, found {seen}"));
    }

    Ok(FimCapturedSystem {
        metadata: FimCaptureMetadata {
            newton_iteration,
            failure_reason,
            dominant_family,
            dominant_item_index,
        },
        layout,
        jacobian: tri.to_csr(),
        rhs,
        equation_scaling,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_system() -> (CsMat<f64>, DVector<f64>) {
        let mut tri = TriMatI::<f64, usize>::new((3, 3));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 2, -1.5e-8);
        tri.add_triplet(1, 1, 2.0);
        tri.add_triplet(2, 0, 1e12);
        tri.add_triplet(2, 2, 8.0);
        (tri.to_csr(), DVector::from_vec(vec![1.0, -2.5, 3e-4]))
    }

    #[test]
    fn capture_round_trips_metadata_layout_rhs_and_triplets() {
        let (jacobian, rhs) = sample_system();
        let metadata = FimCaptureMetadata {
            newton_iteration: 7,
            failure_reason: "dead-state".to_string(),
            dominant_family: "water".to_string(),
            dominant_item_index: 405,
        };
        let layout = Some(FimLinearBlockLayout {
            cell_block_count: 1,
            cell_block_size: 3,
            well_bhp_count: 0,
            perforation_tail_start: 3,
        });

        let dir = std::env::temp_dir().join(format!(
            "fim_capture_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        let equation_scaling = EquationScaling {
            water: vec![10.0],
            oil_component: vec![10.0],
            gas_component: vec![10.0],
            well_constraint: vec![],
            perforation_flow: vec![],
        };
        let _ = fs::remove_dir_all(&dir);
        write_capture(
            &dir,
            3,
            &metadata,
            layout,
            &jacobian,
            &rhs,
            Some(&equation_scaling),
        );

        let loaded = load_captures(&dir).expect("load captures");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(loaded.len(), 1);
        let system = &loaded[0];
        assert_eq!(system.metadata, metadata);
        assert_eq!(system.layout, layout);
        assert_eq!(system.rhs, rhs);
        assert_eq!(system.jacobian.to_dense(), jacobian.to_dense());
        assert_eq!(system.equation_scaling, Some(equation_scaling));
    }

    #[test]
    fn capture_round_trips_none_equation_scaling() {
        let (jacobian, rhs) = sample_system();
        let metadata = FimCaptureMetadata {
            newton_iteration: 1,
            failure_reason: "max-iters".to_string(),
            dominant_family: "water".to_string(),
            dominant_item_index: 0,
        };
        let dir = std::env::temp_dir().join(format!(
            "fim_capture_none_scaling_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        let _ = fs::remove_dir_all(&dir);
        write_capture(&dir, 0, &metadata, None, &jacobian, &rhs, None);

        let loaded = load_captures(&dir).expect("load captures");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].equation_scaling, None);
    }

    #[test]
    fn parse_rejects_wrong_header_and_truncated_triplets() {
        assert!(parse_capture("not-a-capture").is_err());

        let (jacobian, rhs) = sample_system();
        let metadata = FimCaptureMetadata {
            newton_iteration: 0,
            failure_reason: "max-iters".to_string(),
            dominant_family: "oil".to_string(),
            dominant_item_index: 1,
        };
        let dir = std::env::temp_dir().join(format!(
            "fim_capture_trunc_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        let _ = fs::remove_dir_all(&dir);
        write_capture(&dir, 0, &metadata, None, &jacobian, &rhs, None);
        let path = dir.join("fim_capture_00000.txt");
        let content = fs::read_to_string(&path).expect("read");
        let truncated: String = content
            .lines()
            .take(content.lines().count() - 1)
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::remove_dir_all(&dir);

        assert!(parse_capture(&truncated).is_err());
    }
}
