use std::path::Path;
use anyhow::Result;
use serde::Serialize;

pub mod nxs;

/// Detector geometry and image properties returned by the metadata endpoint.
/// Field names match the `ImageMetadata` interface expected by diffrant.
#[derive(Debug, Clone, Serialize)]
pub struct ImageMetadata {
    /// Sample-to-detector distance in mm
    pub panel_distance: f64,
    /// Beam centre in pixels [fast, slow] / [x, y]
    pub beam_center: [f64; 2],
    /// Pixel size in mm
    pub pixel_size: f64,
    /// Detector size in pixels [fast/width, slow/height]
    pub panel_size_fast_slow: [u64; 2],
    /// Bit depth of transferred pixel values (always 16 for NXS)
    pub image_depth: u32,
    /// Pixel value above which pixels are considered masked / untrusted
    pub trusted_range_max: f64,
    /// Beam energy in keV (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beam_energy_kev: Option<f64>,
}

/// Abstraction over different file formats that can supply detector images.
///
/// All methods may perform blocking I/O and should be called from
/// `tokio::task::spawn_blocking` or a similar blocking context.
pub trait Reader: Send + Sync {
    /// Detector metadata (same for all frames in a file).
    fn metadata(&self) -> Result<ImageMetadata>;

    /// Total number of frames in the file.
    fn frame_count(&self) -> Result<usize>;

    /// Read one frame. Returns `(pixels, width, height)` where `pixels` is a
    /// row-major `Vec<u16>` of length `width * height`.
    fn read_frame(&self, frame: usize) -> Result<(Vec<u16>, usize, usize)>;
}

/// Open a file by inspecting its extension and returning the appropriate reader.
///
/// Extend this function to support additional formats: add a new module under
/// `readers/` and match on the extension here.
pub fn open(path: &Path) -> Result<Box<dyn Reader>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "nxs" | "h5" | "hdf5" | "nx5" => Ok(Box::new(nxs::NxsReader::open(path)?)),
        _ => anyhow::bail!(
            "Unsupported file extension '.{ext}'. Supported: nxs, h5, hdf5, nx5"
        ),
    }
}
