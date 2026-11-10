//! External installed icons only. No artwork is embedded in the application.
use eframe::egui;
use std::path::Path;

#[cfg(not(any(windows, target_os = "linux")))]
pub fn load(_: &Path) -> Option<egui::ColorImage> {
    None
}

#[cfg(windows)]
pub fn load(path: &Path) -> Option<egui::ColorImage> {
    use std::{
        mem::{size_of, zeroed},
        os::windows::ffi::OsStrExt,
        ptr,
    };
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        UI::{Shell::ExtractIconExW, WindowsAndMessaging::*},
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut icon = ptr::null_mut();
        if ExtractIconExW(wide.as_ptr(), 0, &mut icon, ptr::null_mut(), 1) == 0 || icon.is_null() {
            return None;
        }
        let mut info: ICONINFO = zeroed();
        if GetIconInfo(icon, &mut info) == 0 {
            DestroyIcon(icon);
            return None;
        }
        let result = (|| {
            if info.hbmColor.is_null() {
                return None;
            }
            let mut bitmap: BITMAP = zeroed();
            if GetObjectW(
                info.hbmColor,
                size_of::<BITMAP>() as i32,
                &mut bitmap as *mut _ as _,
            ) == 0
            {
                return None;
            }
            let (w, h) = (bitmap.bmWidth, bitmap.bmHeight);
            if w <= 0 || h <= 0 || w > 512 || h > 512 {
                return None;
            }
            let dc = GetDC(ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut header: BITMAPINFO = zeroed();
            header.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            header.bmiHeader.biWidth = w;
            header.bmiHeader.biHeight = -h;
            header.bmiHeader.biPlanes = 1;
            header.bmiHeader.biBitCount = 32;
            header.bmiHeader.biCompression = BI_RGB;
            let mut bytes = vec![0u8; (w * h * 4) as usize];
            let rows = GetDIBits(
                dc,
                info.hbmColor,
                0,
                h as u32,
                bytes.as_mut_ptr() as _,
                &mut header,
                DIB_RGB_COLORS,
            );
            if rows != 0 && bytes.chunks_exact(4).all(|p| p[3] == 0) {
                let mut mask = vec![0u8; bytes.len()];
                if !info.hbmMask.is_null()
                    && GetDIBits(
                        dc,
                        info.hbmMask,
                        0,
                        h as u32,
                        mask.as_mut_ptr() as _,
                        &mut header,
                        DIB_RGB_COLORS,
                    ) != 0
                {
                    for (p, m) in bytes.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
                        p[3] = if m[0] == 0 { 255 } else { 0 };
                    }
                } else {
                    for p in bytes.chunks_exact_mut(4) {
                        p[3] = 255;
                    }
                }
            }
            ReleaseDC(ptr::null_mut(), dc);
            if rows == 0 {
                return None;
            }
            Some(egui::ColorImage {
                size: [w as usize, h as usize],
                pixels: bytes
                    .chunks_exact(4)
                    .map(|p| egui::Color32::from_rgba_premultiplied(p[2], p[1], p[0], p[3]))
                    .collect(),
            })
        })();
        if !info.hbmColor.is_null() {
            DeleteObject(info.hbmColor);
        }
        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask);
        }
        DestroyIcon(icon);
        result
    }
}

#[cfg(target_os = "linux")]
pub fn load(path: &Path) -> Option<egui::ColorImage> {
    use std::path::PathBuf;
    let name = path.file_name()?.to_string_lossy();
    let mut roots: Vec<PathBuf> = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .into_iter()
        .collect();
    roots.extend(std::env::split_paths(
        &std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    ));
    let mut names = vec![name.to_string()];
    for root in &roots {
        if let Ok(entries) = std::fs::read_dir(root.join("applications")) {
            for entry in entries.flatten().take(1024) {
                let file = entry.path();
                if file.extension().is_none_or(|e| e != "desktop")
                    || !file.metadata().is_ok_and(|m| m.len() <= 64 * 1024)
                {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(file) {
                    let section = text
                        .split("[Desktop Entry]")
                        .nth(1)
                        .unwrap_or("")
                        .split("\n[")
                        .next()
                        .unwrap_or("");
                    let matches =
                        section
                            .lines()
                            .filter_map(|l| l.strip_prefix("Exec="))
                            .any(|v| {
                                v.split_whitespace().next().is_some_and(|p| {
                                    Path::new(p.trim_matches('"'))
                                        .file_name()
                                        .is_some_and(|n| n == name.as_ref())
                                })
                            });
                    if matches {
                        if let Some(icon) = section.lines().find_map(|l| l.strip_prefix("Icon=")) {
                            names.insert(0, icon.trim().to_owned());
                        }
                    }
                }
            }
        }
    }
    let mut candidates = vec![];
    for name in names {
        if Path::new(&name).is_absolute() {
            candidates.push(PathBuf::from(name));
            continue;
        }
        let file = if name.ends_with(".png") {
            name
        } else {
            format!("{name}.png")
        };
        for root in &roots {
            candidates.push(root.join("pixmaps").join(&file));
            for theme in ["hicolor", "Adwaita", "Yaru", "breeze"] {
                for size in ["48x48", "64x64", "32x32", "128x128", "256x256"] {
                    candidates.push(
                        root.join("icons")
                            .join(theme)
                            .join(size)
                            .join("apps")
                            .join(&file),
                    );
                }
            }
        }
    }
    for file in candidates {
        if file.extension().is_none_or(|e| e != "png")
            || !file.metadata().is_ok_and(|m| m.len() <= 2 * 1024 * 1024)
        {
            continue;
        }
        let Ok(mut reader) = image::ImageReader::open(file) else {
            continue;
        };
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(512);
        limits.max_image_height = Some(512);
        limits.max_alloc = Some(4 * 1024 * 1024);
        reader.limits(limits);
        if let Ok(decoded) = reader.decode() {
            let rgba = decoded.to_rgba8();
            return Some(egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            ));
        }
    }
    None
}
