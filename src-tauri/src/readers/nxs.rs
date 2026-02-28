//! Reader for NXmx NeXus / HDF5 files.
//!
//! Adapted from `serious/backend/src/routes/api.rs` (`read_hdf5_frame` and
//! `read_hdf5_metadata`).  The main changes are:
//!
//! - No axum/HTTP coupling — returns plain Rust types.
//! - Metadata field `panel_distance_mm` matches diffrant's `ImageMetadata`.
//! - `NxsReader` stores only the path; the HDF5 file is opened per call so the
//!   struct is `Send + Sync` without a mutex around the file handle.

use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use tracing::debug;

use super::{ImageMetadata, Reader};

pub struct NxsReader {
    path: PathBuf,
}

impl NxsReader {
    /// Validate the file is readable, then return a reader for it.
    pub fn open(path: &Path) -> Result<Self> {
        // Quick open-and-close to surface errors early.
        let _file = hdf5::File::open(path)
            .map_err(|e| anyhow!("Cannot open {}: {e}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl Reader for NxsReader {
    fn frame_count(&self) -> Result<usize> {
        let file = hdf5::File::open(&self.path)?;
        let dataset = file.dataset("entry/data/data")?;
        let shape = dataset.shape();
        if shape.len() != 3 {
            anyhow::bail!("Expected 3D dataset, got {}D", shape.len());
        }
        Ok(shape[0])
    }

    fn metadata(&self) -> Result<ImageMetadata> {
        read_nxs_metadata(&self.path)
    }

    fn read_frame(&self, frame: usize) -> Result<(Vec<u16>, usize, usize)> {
        read_nxs_frame(&self.path, frame)
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn read_nxs_frame(path: &Path, frame_idx: usize) -> Result<(Vec<u16>, usize, usize)> {
    use std::time::Instant;
    let t_total = Instant::now();

    let file = hdf5::File::open(path)
        .map_err(|e| anyhow!("Failed to open {}: {e}", path.display()))?;

    let dataset = file
        .dataset("entry/data/data")
        .map_err(|e| anyhow!("Failed to open dataset entry/data/data: {e}"))?;

    let shape = dataset.shape();
    if shape.len() != 3 {
        anyhow::bail!("Expected 3D dataset, got {}D", shape.len());
    }
    if frame_idx >= shape[0] {
        anyhow::bail!(
            "Frame index {frame_idx} out of range (dataset has {} frames)",
            shape[0]
        );
    }

    let height = shape[1];
    let width = shape[2];
    let dtype_desc = format!("{:?}", dataset.dtype()?.to_descriptor()?);

    let t0 = Instant::now();
    let pixels: Vec<u16> = if dataset.dtype()?.is::<u16>() {
        let frame = dataset.read_slice_2d::<u16, _>((frame_idx, .., ..))?;
        frame.into_raw_vec_and_offset().0
    } else if dataset.dtype()?.is::<i32>() {
        let frame = dataset.read_slice_2d::<i32, _>((frame_idx, .., ..))?;
        frame.iter().map(|&v| v as i16 as u16).collect()
    } else if dataset.dtype()?.is::<u32>() {
        let frame = dataset.read_slice_2d::<u32, _>((frame_idx, .., ..))?;
        frame.iter().map(|&v| v as i32 as i16 as u16).collect()
    } else if dataset.dtype()?.is::<i16>() {
        let frame = dataset.read_slice_2d::<i16, _>((frame_idx, .., ..))?;
        frame.iter().map(|&v| v as u16).collect()
    } else {
        anyhow::bail!("Unsupported pixel dtype: {dtype_desc}");
    };
    debug!(
        elapsed_ms = t0.elapsed().as_millis(),
        dtype = dtype_desc,
        width,
        height,
        "nxs: frame read + convert"
    );
    debug!(
        total_ms = t_total.elapsed().as_millis(),
        "nxs: read_nxs_frame total"
    );

    Ok((pixels, width, height))
}

fn read_nxs_metadata(path: &Path) -> Result<ImageMetadata> {
    use std::time::Instant;
    let t_total = Instant::now();

    let file = hdf5::File::open(path)
        .map_err(|e| anyhow!("Failed to open {}: {e}", path.display()))?;

    let dataset = file.dataset("entry/data/data")?;
    let shape = dataset.shape();
    let (width, height) = if shape.len() == 3 {
        (shape[2] as u64, shape[1] as u64)
    } else {
        anyhow::bail!("Expected 3D dataset, got {}D", shape.len());
    };

    let detector = file.group("entry/instrument/detector")?;

    // Distance: try "distance" then "detector_distance"; read value + units, convert to mm
    let panel_distance = ["distance", "detector_distance"]
        .iter()
        .find_map(|name| {
            let ds = detector.dataset(name).ok()?;
            // Try scalar first, then 1-element array (shape {1})
            let raw = ds
                .read_scalar::<f64>()
                .or_else(|_| ds.read_scalar::<f32>().map(|v| v as f64))
                .or_else(|_| ds.read_1d::<f64>().map(|a| a[0]))
                .or_else(|_| ds.read_1d::<f32>().map(|a| a[0] as f64))
                .ok()?;
            let units = read_dataset_attr_string(&ds, "units").unwrap_or_else(|| "m".to_owned());
            let mm = match units.to_lowercase().as_str() {
                "mm" => raw,
                "cm" => raw * 10.0,
                "m" | _ => raw * 1000.0,
            };
            Some(mm)
        })
        .unwrap_or(0.0);

    // Pixel size: stored in metres, convert to mm
    let pixel_size = read_scalar_f64(&detector, "x_pixel_size")
        .map(|v| v * 1000.0)
        .unwrap_or(0.075);

    // Beam centre in pixels
    let beam_cx = read_scalar_f64(&detector, "beam_center_x").unwrap_or(width as f64 / 2.0);
    let beam_cy = read_scalar_f64(&detector, "beam_center_y").unwrap_or(height as f64 / 2.0);

    // Beam energy: prefer direct eV, fall back to wavelength
    let beam_energy_kev = file.group("entry/instrument/beam").ok().and_then(|beam| {
        if let Some(ev) = read_scalar_f64(&beam, "incident_energy") {
            return Some(ev / 1000.0);
        }
        let ds = beam.dataset("incident_wavelength").ok()?;
        let raw = ds
            .read_scalar::<f64>()
            .or_else(|_| ds.read_scalar::<f32>().map(|v| v as f64))
            .ok()?;
        let units =
            read_dataset_attr_string(&ds, "units").unwrap_or_else(|| "angstrom".to_owned());
        let angstrom = match units.to_lowercase().as_str() {
            "angstrom" | "angstroms" | "a" | "\u{00c5}" => raw,
            "nm" => raw * 10.0,
            "m" => raw * 1e10,
            _ => raw,
        };
        Some(wavelength_to_energy_kev(angstrom))
    });

    // Trusted range max
    let trusted_range_max = detector
        .group("detectorSpecific")
        .ok()
        .and_then(|ds| read_scalar_f64(&ds, "countrate_correction_count_cutoff"))
        .or_else(|| read_scalar_f64(&detector, "saturation_value"))
        .unwrap_or((u16::MAX - 1) as f64);

    debug!(
        total_ms = t_total.elapsed().as_millis(),
        "nxs: read_nxs_metadata total"
    );

    Ok(ImageMetadata {
        panel_distance_mm: panel_distance,
        beam_center: [beam_cx, beam_cy],
        pixel_size,
        panel_size_fast_slow: [width, height],
        image_depth: 16,
        trusted_range_max,
        beam_energy_kev,
    })
}

fn read_scalar_f64(group: &hdf5::Group, name: &str) -> Option<f64> {
    let ds = group.dataset(name).ok()?;
    ds.read_scalar::<f64>()
        .or_else(|_| ds.read_scalar::<f32>().map(|v| v as f64))
        .ok()
}

fn read_dataset_attr_string(ds: &hdf5::Dataset, attr_name: &str) -> Option<String> {
    use hdf5::types::{FixedAscii, VarLenAscii, VarLenUnicode};
    let attr = ds.attr(attr_name).ok()?;
    attr.read_scalar::<VarLenUnicode>()
        .map(|s| s.as_str().to_owned())
        .or_else(|_| {
            attr.read_scalar::<VarLenAscii>()
                .map(|s| s.as_str().to_owned())
        })
        .or_else(|_| {
            attr.read_scalar::<FixedAscii<64>>()
                .map(|s| s.as_str().to_owned())
        })
        .ok()
}

/// E (keV) = hc / λ, with hc = 12.398419843 keV·Å
fn wavelength_to_energy_kev(wavelength_angstrom: f64) -> f64 {
    12.398_419_843 / wavelength_angstrom
}
