//! Lenient `.wsz` (ZIP) and loose directory skin loader with automatic Clean-Room fallback.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use image::ImageFormat;
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::cleanroom::{SkinReport, SpriteImage, WszSkin};
use crate::pledit::parse_pledit_txt;
use crate::region::parse_region_txt;
use crate::viscolor::parse_viscolor_txt;

/// Errors returned when loading or exporting a `.wsz` skin.
#[derive(Debug, Error)]
pub enum SkinError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("image encoding/decoding error: {0}")]
    Image(#[from] image::ImageError),
}

/// Loads a Winamp 2.x skin from either a `.wsz`/`.zip` archive or an extracted directory.
/// Missing or corrupt assets gracefully fall back to the 100% Clean-Room default skin.
pub fn load_skin(path: impl AsRef<Path>) -> Result<WszSkin, SkinError> {
    let path = path.as_ref();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Custom Skin")
        .to_string();

    if path.is_dir() {
        let mut entries = HashMap::new();
        collect_dir_entries(path, &mut entries)?;
        Ok(assemble_skin_lenient(name, &entries))
    } else {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;
        load_from_zip_archive(name, &mut archive)
    }
}

/// Loads a `.wsz` skin directly from in-memory ZIP bytes.
pub fn load_skin_from_wsz_bytes(
    name: impl Into<String>,
    bytes: &[u8],
) -> Result<WszSkin, SkinError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    load_from_zip_archive(name.into(), &mut archive)
}

fn load_from_zip_archive<R: Read + std::io::Seek>(
    name: String,
    archive: &mut ZipArchive<R>,
) -> Result<WszSkin, SkinError> {
    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        // Lenient matching: normalize path separators and use lowercase file basename
        // so skins packed inside a top-level folder still load cleanly!
        let normalized = entry.name().replace('\\', "/");
        let Some(basename) = normalized.rsplit('/').next() else {
            continue;
        };
        let key = basename.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_ok() {
            entries.insert(key, data);
        }
    }
    Ok(assemble_skin_lenient(name, &entries))
}

fn collect_dir_entries(dir: &Path, out: &mut HashMap<String, Vec<u8>>) -> Result<(), SkinError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|s| s.to_str())
        {
            let key = name.to_ascii_lowercase();
            if let Ok(bytes) = fs::read(&path) {
                out.insert(key, bytes);
            }
        }
    }
    Ok(())
}

fn assemble_skin_lenient(name: String, files: &HashMap<String, Vec<u8>>) -> WszSkin {
    let mut skin = WszSkin::clean_room_default();
    skin.name = name.clone();
    skin.report = SkinReport {
        source_name: name,
        loaded_files: Vec::new(),
        fallback_assets: Vec::new(),
        warnings: Vec::new(),
    };

    macro_rules! load_sprite {
        ($field:ident, $names:expr) => {
            match try_load_sprite(files, $names) {
                Ok(Some((matched, sprite))) => {
                    skin.sprites.$field = sprite;
                    skin.report.loaded_files.push(matched);
                }
                Ok(None) => {
                    skin.report.fallback_assets.push($names[0].to_string());
                }
                Err((matched, msg)) => {
                    skin.report.warnings.push(format!(
                        "failed to decode {matched}: {msg}; using clean-room fallback"
                    ));
                    skin.report.fallback_assets.push(matched);
                }
            }
        };
    }

    load_sprite!(main, &["main.bmp", "main.png"]);
    load_sprite!(titlebar, &["titlebar.bmp", "titlebar.png"]);
    load_sprite!(cbuttons, &["cbuttons.bmp", "cbuttons.png"]);
    load_sprite!(volume, &["volume.bmp", "volume.png"]);
    load_sprite!(balance, &["balance.bmp", "balance.png"]);
    load_sprite!(posbar, &["posbar.bmp", "posbar.png"]);
    load_sprite!(shufrep, &["shufrep.bmp", "shufrep.png"]);
    load_sprite!(monoster, &["monoster.bmp", "monoster.png"]);
    load_sprite!(playpaus, &["playpaus.bmp", "playpaus.png"]);
    load_sprite!(
        numbers,
        &["nums_ex.bmp", "numbers.bmp", "nums_ex.png", "numbers.png"]
    );
    load_sprite!(text, &["text.bmp", "text.png"]);
    load_sprite!(eqmain, &["eqmain.bmp", "eqmain.png"]);
    load_sprite!(eq_ex, &["eq_ex.bmp", "eq_ex.png"]);
    load_sprite!(pledit, &["pledit.bmp", "pledit.png"]);

    if let Some(raw) = files.get("viscolor.txt") {
        skin.vis_colors = parse_viscolor_txt(&String::from_utf8_lossy(raw));
        skin.report.loaded_files.push("viscolor.txt".to_string());
    } else {
        skin.report.fallback_assets.push("viscolor.txt".to_string());
    }

    if let Some(raw) = files.get("pledit.txt") {
        skin.pledit_style = parse_pledit_txt(&String::from_utf8_lossy(raw));
        skin.report.loaded_files.push("pledit.txt".to_string());
    } else {
        skin.report.fallback_assets.push("pledit.txt".to_string());
    }

    if let Some(raw) = files.get("region.txt") {
        skin.regions = parse_region_txt(&String::from_utf8_lossy(raw));
        skin.report.loaded_files.push("region.txt".to_string());
    }

    skin
}

fn try_load_sprite(
    files: &HashMap<String, Vec<u8>>,
    candidates: &[&str],
) -> Result<Option<(String, SpriteImage)>, (String, String)> {
    for &name in candidates {
        if let Some(bytes) = files.get(name) {
            match image::load_from_memory(bytes) {
                Ok(dyn_img) => {
                    let rgba_img = dyn_img.to_rgba8();
                    let (width, height) = rgba_img.dimensions();
                    return Ok(Some((
                        name.to_string(),
                        SpriteImage {
                            width,
                            height,
                            rgba: rgba_img.into_raw(),
                        },
                    )));
                }
                Err(err) => return Err((name.to_string(), err.to_string())),
            }
        }
    }
    Ok(None)
}

/// Exports a [`WszSkin`] into an in-memory `.wsz` (ZIP) byte archive containing BMP sprites
/// and text definitions (`viscolor.txt`, `pledit.txt`).
pub fn write_wsz_bytes(skin: &WszSkin) -> Result<Vec<u8>, SkinError> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let sprites: [(&str, &SpriteImage); 14] = [
            ("main.bmp", &skin.sprites.main),
            ("titlebar.bmp", &skin.sprites.titlebar),
            ("cbuttons.bmp", &skin.sprites.cbuttons),
            ("volume.bmp", &skin.sprites.volume),
            ("balance.bmp", &skin.sprites.balance),
            ("posbar.bmp", &skin.sprites.posbar),
            ("shufrep.bmp", &skin.sprites.shufrep),
            ("monoster.bmp", &skin.sprites.monoster),
            ("playpaus.bmp", &skin.sprites.playpaus),
            ("nums_ex.bmp", &skin.sprites.numbers),
            ("text.bmp", &skin.sprites.text),
            ("eqmain.bmp", &skin.sprites.eqmain),
            ("eq_ex.bmp", &skin.sprites.eq_ex),
            ("pledit.bmp", &skin.sprites.pledit),
        ];

        for (filename, sprite) in sprites {
            let mut bmp_buf = Cursor::new(Vec::new());
            let rgba_buf =
                image::RgbaImage::from_raw(sprite.width, sprite.height, sprite.rgba.clone())
                    .ok_or_else(|| {
                        SkinError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid sprite dimensions",
                        ))
                    })?;
            image::DynamicImage::ImageRgba8(rgba_buf).write_to(&mut bmp_buf, ImageFormat::Bmp)?;
            zip.start_file(filename, opts)?;
            zip.write_all(&bmp_buf.into_inner())?;
        }

        // Write viscolor.txt
        let mut vis_txt = String::new();
        for c in &skin.vis_colors.colors {
            vis_txt.push_str(&format!("{},{},{}\n", c.r, c.g, c.b));
        }
        zip.start_file("viscolor.txt", opts)?;
        zip.write_all(vis_txt.as_bytes())?;

        // Write pledit.txt
        let pl = &skin.pledit_style;
        let pledit_txt = format!(
            "[Text]\nNormal=#{:02X}{:02X}{:02X}\nCurrent=#{:02X}{:02X}{:02X}\nNormalBG=#{:02X}{:02X}{:02X}\nSelectedBG=#{:02X}{:02X}{:02X}\nFont={}\n",
            pl.normal.r,
            pl.normal.g,
            pl.normal.b,
            pl.current.r,
            pl.current.g,
            pl.current.b,
            pl.normal_bg.r,
            pl.normal_bg.g,
            pl.normal_bg.b,
            pl.selected_bg.r,
            pl.selected_bg.g,
            pl.selected_bg.b,
            pl.font
        );
        zip.start_file("pledit.txt", opts)?;
        zip.write_all(pledit_txt.as_bytes())?;

        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_cleanroom_skin_through_wsz_zip_archive() {
        let default_skin = WszSkin::clean_room_default();
        let wsz_bytes = write_wsz_bytes(&default_skin).expect("write wsz");
        let reloaded = load_skin_from_wsz_bytes("pilimao_test.wsz", &wsz_bytes).expect("load wsz");

        assert_eq!(reloaded.sprites.main.width, 275);
        assert_eq!(reloaded.sprites.main.height, 116);
        assert!(reloaded.report.warnings.is_empty());
        assert!(reloaded.report.fallback_assets.is_empty());
        assert_eq!(reloaded.vis_colors, default_skin.vis_colors);
        assert_eq!(
            reloaded.pledit_style.normal,
            default_skin.pledit_style.normal
        );
    }

    #[test]
    fn lenient_loader_falls_back_on_partial_or_corrupt_wsz() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default();
            zip.start_file("NestedFolder/MAIN.BMP", opts).unwrap();
            zip.write_all(b"not a valid bmp").unwrap();
            zip.start_file("NestedFolder/VISCOLOR.TXT", opts).unwrap();
            zip.write_all(b"1,2,3\n4,5,6\n").unwrap();
            zip.finish().unwrap();
        }
        let skin = load_skin_from_wsz_bytes("broken.wsz", &cursor.into_inner()).expect("lenient");
        // Corrupt main.bmp fell back to 275x116 clean-room sprite with warning
        assert_eq!(skin.sprites.main.width, 275);
        assert_eq!(skin.report.warnings.len(), 1);
        assert_eq!(
            skin.vis_colors.background(),
            crate::viscolor::Rgb::new(1, 2, 3)
        );
    }
}
