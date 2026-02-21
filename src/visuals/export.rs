use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::tasks::futures_lite::future;
use symbios_ground::HeightMap;

use crate::core::config::{ExportStatus, ExportTask, TerrainConfig};

// ---------------------------------------------------------------------------
// Platform-agnostic file I/O
// ---------------------------------------------------------------------------

/// Write a UTF-8 string to `exports/<filename>`, creating the directory if
/// needed. Returns `Err` with a human-readable message on failure.
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

/// WASM: delegate to [`save_file_binary`] via UTF-8 byte conversion.
#[cfg(target_arch = "wasm32")]
pub fn save_file(filename: &str, content: &str) -> Result<(), String> {
    save_file_binary(filename, content.as_bytes())
}

/// Write raw bytes to `exports/<filename>`, creating the directory if needed.
/// Returns `Err` with a human-readable message on failure.
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

/// WASM: wrap bytes in a Blob, create an object URL, trigger a synthetic
/// anchor click to initiate the browser download, then schedule URL revocation
/// after 60 seconds to avoid unbounded memory growth from repeated exports.
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
    // Attach to the document body before firing the click event.  Unattached
    // anchor clicks work in Chrome/V8 but are silently ignored by Safari and
    // some restrictive browser environments, causing the download to never
    // start.  Remove immediately after — the element is only needed long
    // enough to dispatch the event.
    let body = document.body().ok_or("No document body")?;
    body.append_child(&a)
        .map_err(|e| format!("DOM append: {e:?}"))?;
    a.click();
    body.remove_child(&a)
        .map_err(|e| format!("DOM remove: {e:?}"))?;
    // Revoke the blob URL after a 60-second delay. Revoking synchronously
    // would destroy the URL before Firefox/Safari finish their async download
    // initiation. A 60-second window is ample for any browser, and prevents
    // unbounded memory growth from repeated large (100+ MB OBJ) exports.
    //
    // `once_into_js` transfers ownership to JS: when the callback fires the JS
    // GC drops the Rust-side closure, so no WASM linear-memory leak occurs.
    // The old `Closure::once` + `forget()` pattern permanently leaked the
    // closure allocation on every export.
    let url_to_revoke = url.clone();
    let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url_to_revoke).ok();
    });
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 60_000)
        .ok();
    Ok(())
}

// ---------------------------------------------------------------------------
// Issue #6: 16-bit PNG heightmap export
// ---------------------------------------------------------------------------

/// Spawn a background task that encodes the heightmap as a 16-bit greyscale
/// PNG and writes it to disk, using the same `AsyncComputeTaskPool` pattern
/// as OBJ export so the main thread is never blocked by the deflate encoder.
///
/// Heights are normalised to the current `[min, max]` range so the full
/// 16-bit dynamic range (`[0, 65535]`) is always utilised. `status` is set to
/// `Exporting` immediately so the UI can show a spinner.
pub fn spawn_png_export(hm: HeightMap, task: &mut ExportTask, status: &mut ExportStatus) {
    let pool = AsyncComputeTaskPool::get();
    let t = pool.spawn(async move {
        use image::{ImageBuffer, Luma};
        use std::io::Cursor;

        let w = hm.width() as u32;
        let h = hm.height() as u32;

        // Find the current height range for full 16-bit dynamic range.
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
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| format!("PNG encode: {e}"))?;
        save_file_binary("heightmap.png", &cursor.into_inner())?;
        Ok("heightmap.png".into())
    });
    task.0 = Some(t);
    *status = ExportStatus::Exporting;
}

// ---------------------------------------------------------------------------
// Issue #7: OBJ mesh export
// ---------------------------------------------------------------------------

/// Spawn a background task that serialises the heightmap to OBJ and writes it
/// to disk, mirroring the AsyncComputeTaskPool pattern used for generation.
/// Sets `ExportStatus::Exporting` immediately so the UI can show a spinner.
pub fn spawn_obj_export(hm: HeightMap, task: &mut ExportTask, status: &mut ExportStatus) {
    let pool = AsyncComputeTaskPool::get();
    let t = pool.spawn(async move {
        // A 2048×2048 OBJ is ~700 MB of text. WASM runs in a 32-bit address
        // space; the peak allocation during string growth (old buf + new buf)
        // exceeds available memory on that target. Reject large grids early
        // with a clear message rather than silently crashing the page.
        #[cfg(target_arch = "wasm32")]
        {
            let cells = hm.width() * hm.height();
            if cells > 512 * 512 {
                return Err(format!(
                    "OBJ export on the web is limited to 512×512 grids \
                     (current: {}×{}). Reduce the grid size and retry.",
                    hm.width(),
                    hm.height()
                ));
            }
        }
        let obj = heightmap_to_obj(&hm);
        save_file("terrain.obj", &obj)?;
        Ok("terrain.obj".into())
    });
    task.0 = Some(t);
    *status = ExportStatus::Exporting;
}

/// Poll the in-flight OBJ export task and update `ExportStatus` when done.
pub fn poll_export_task(mut task: ResMut<ExportTask>, mut status: ResMut<ExportStatus>) {
    let Some(ref mut t) = task.0 else { return };
    if let Some(result) = future::block_on(future::poll_once(t)) {
        *status = match result {
            Ok(filename) => ExportStatus::Done(filename),
            Err(e) => ExportStatus::Error(e),
        };
        task.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbios_ground::HeightMap;

    #[test]
    fn obj_uv_no_nan_for_1x1_map() {
        // A 1×1 heightmap has w-1 == 0; dividing by it previously produced NaN UVs.
        let hm = HeightMap::new(1, 1, 1.0);
        let obj = heightmap_to_obj(&hm);
        for line in obj.lines() {
            if let Some(coords) = line.strip_prefix("vt ") {
                for token in coords.split_whitespace() {
                    let v: f32 = token
                        .parse()
                        .expect("UV coordinate should be a valid float");
                    assert!(v.is_finite(), "NaN/Inf UV in OBJ output: {line}");
                }
            }
        }
    }
}

/// Serialise a heightmap to the Wavefront OBJ format.
///
/// Emits vertex positions (`v`), UV coordinates (`vt`, normalised to `[0, 1]`),
/// per-vertex normals computed via central differences (`vn`), and triangulated
/// quad faces (`f`) with combined vertex/UV/normal indices. The output is
/// suitable for import into any DCC tool that understands OBJ.
fn heightmap_to_obj(hm: &HeightMap) -> String {
    let w = hm.width();
    let h = hm.height();
    // ~84 bytes per vertex/UV/normal triple + ~160 bytes per quad's two face
    // lines. Accurate pre-allocation avoids repeated doubling for mid-size
    // grids; capped at 128 MiB so we never pre-commit huge RAM for large ones.
    let estimated = w * h * 84 + w.saturating_sub(1) * h.saturating_sub(1) * 160;
    let mut out = String::with_capacity(estimated.min(128 * 1024 * 1024));

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
            let u = if w > 1 {
                x as f32 / (w - 1) as f32
            } else {
                0.0
            };
            let v = if h > 1 {
                z as f32 / (h - 1) as f32
            } else {
                0.0
            };
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

/// Export the current [`TerrainConfig`] plus derived metadata as a
/// pretty-printed JSON file (`exports/terrain.json`).
///
/// If a heightmap is provided, the metadata includes the actual grid
/// dimensions, world extents, and measured height range. Otherwise, the values
/// are estimated from `config` alone. `status` is updated to reflect success
/// or failure.
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
