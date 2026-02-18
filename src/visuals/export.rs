use bevy::prelude::*;
use symbios_ground::HeightMap;

use crate::core::config::{ExportStatus, TerrainConfig};

// ---------------------------------------------------------------------------
// Platform-agnostic file I/O
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(filename: &str, content: &str) -> Result<(), String> {
    use std::{fs, path::Path};
    let dir = Path::new("exports");
    fs::create_dir_all(dir).map_err(|e| format!("mkdir exports: {e}"))?;
    let path = dir.join(filename);
    fs::write(&path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    info!("Exported: {}", path.display());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(filename: &str, content: &str) -> Result<(), String> {
    save_file_binary(filename, content.as_bytes())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file_binary(filename: &str, bytes: &[u8]) -> Result<(), String> {
    use std::{fs, path::Path};
    let dir = Path::new("exports");
    fs::create_dir_all(dir).map_err(|e| format!("mkdir exports: {e}"))?;
    let path = dir.join(filename);
    fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
    info!("Exported: {}", path.display());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn save_file_binary(filename: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let arr = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    arr.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr);
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.type_("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("blob: {e:?}"))?;
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("url: {e:?}"))?;
    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| format!("create a: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not anchor")?;
    a.set_href(&url);
    a.set_download(filename);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

// ---------------------------------------------------------------------------
// Issue #6: 16-bit PNG heightmap export
// ---------------------------------------------------------------------------

pub fn export_heightmap_png(hm: &HeightMap, status: &mut ExportStatus) {
    use image::{ImageBuffer, Luma};
    use std::io::Cursor;

    let w = hm.width() as u32;
    let h = hm.height() as u32;

    // Find the current height range for full 16-bit dynamic range
    let min = hm.data().iter().cloned().fold(f32::INFINITY, f32::min);
    let max = hm.data().iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(f32::EPSILON);

    let mut img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::new(w, h);
    for z in 0..hm.height() {
        for x in 0..hm.width() {
            let val = (hm.get(x, z) - min) / range;
            img.put_pixel(x as u32, z as u32, Luma([(val * 65535.0) as u16]));
        }
    }

    let mut cursor = Cursor::new(Vec::new());
    match img.write_to(&mut cursor, image::ImageFormat::Png) {
        Ok(()) => match save_file_binary("heightmap.png", &cursor.into_inner()) {
            Ok(()) => *status = ExportStatus::Done("heightmap.png".into()),
            Err(e) => *status = ExportStatus::Error(e),
        },
        Err(e) => *status = ExportStatus::Error(format!("PNG encode: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Issue #7: OBJ mesh export
// ---------------------------------------------------------------------------

pub fn export_obj(hm: &HeightMap, status: &mut ExportStatus) {
    let obj = heightmap_to_obj(hm);
    match save_file("terrain.obj", &obj) {
        Ok(()) => *status = ExportStatus::Done("terrain.obj".into()),
        Err(e) => *status = ExportStatus::Error(e),
    }
}

fn heightmap_to_obj(hm: &HeightMap) -> String {
    let w = hm.width();
    let h = hm.height();
    let mut out = String::with_capacity(w * h * 64);

    out.push_str("# symbios-ground-lab terrain export\n");

    // Vertices
    for z in 0..h {
        for x in 0..w {
            let wx = x as f32 * hm.scale();
            let wy = hm.get(x, z);
            let wz = z as f32 * hm.scale();
            out.push_str(&format!("v {wx:.4} {wy:.4} {wz:.4}\n"));
        }
    }

    // UV coordinates
    for z in 0..h {
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let v = z as f32 / (h - 1) as f32;
            out.push_str(&format!("vt {u:.6} {v:.6}\n"));
        }
    }

    // Normals
    for z in 0..h {
        for x in 0..w {
            let [nx, ny, nz] = hm.get_normal_at(x as f32 * hm.scale(), z as f32 * hm.scale());
            out.push_str(&format!("vn {nx:.6} {ny:.6} {nz:.6}\n"));
        }
    }

    // Faces (1-indexed, two triangles per quad)
    for z in 0..h - 1 {
        for x in 0..w - 1 {
            let tl = z * w + x + 1;
            let tr = z * w + (x + 1) + 1;
            let bl = (z + 1) * w + x + 1;
            let br = (z + 1) * w + (x + 1) + 1;
            out.push_str(&format!(
                "f {tl}/{tl}/{tl} {bl}/{bl}/{bl} {br}/{br}/{br}\n\
                 f {tl}/{tl}/{tl} {br}/{br}/{br} {tr}/{tr}/{tr}\n"
            ));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Issue #8: JSON config/metadata export
// ---------------------------------------------------------------------------

pub fn export_json(config: &TerrainConfig, hm: Option<&HeightMap>, status: &mut ExportStatus) {
    #[derive(serde::Serialize)]
    struct Export<'a> {
        config: &'a TerrainConfig,
        metadata: Metadata,
    }
    #[derive(serde::Serialize)]
    struct Metadata {
        grid_size: usize,
        world_width: f32,
        world_depth: f32,
        height_range: Option<[f32; 2]>,
    }

    let metadata = if let Some(hm) = hm {
        let min = hm.data().iter().cloned().fold(f32::INFINITY, f32::min);
        let max = hm.data().iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        Metadata {
            grid_size: hm.width(),
            world_width: hm.world_width(),
            world_depth: hm.world_depth(),
            height_range: Some([min, max]),
        }
    } else {
        Metadata {
            grid_size: config.grid_size as usize,
            world_width: config.grid_size as f32 * config.cell_scale,
            world_depth: config.grid_size as f32 * config.cell_scale,
            height_range: None,
        }
    };

    let payload = Export { config, metadata };
    match serde_json::to_string_pretty(&payload) {
        Ok(json) => match save_file("terrain.json", &json) {
            Ok(()) => *status = ExportStatus::Done("terrain.json".into()),
            Err(e) => *status = ExportStatus::Error(e),
        },
        Err(e) => *status = ExportStatus::Error(format!("JSON encode: {e}")),
    }
}
