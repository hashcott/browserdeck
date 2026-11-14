//! The icon the system shows for a file (an app, a shortcut, a document), as a PNG for a key.
use std::path::Path;

/// None: no icon could be read.
#[cfg(windows)]
pub fn png(path: &Path) -> Option<Vec<u8>> {
    let (w, h, rgba) = windows_icon(path)?;
    let (w, h, rgba) = trim(w, h, &rgba)?;
    encode(w, h, &rgba)
}

/// An app bundle's own icon file (named in its Info.plist), converted by `sips`. Anything else: None.
#[cfg(target_os = "macos")]
pub fn png(path: &Path) -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};
    let run = |c: &mut Command| c.stdin(Stdio::null()).stderr(Stdio::null()).output().ok().filter(|o| o.status.success());
    let plist = path.join("Contents/Info.plist");
    let name = run(Command::new("plutil").args(["-extract", "CFBundleIconFile", "raw", "-o", "-"]).arg(&plist))?.stdout;
    let mut icns = path.join("Contents/Resources").join(String::from_utf8(name).ok()?.trim());
    if icns.extension().is_none() {
        icns.set_extension("icns");
    }
    let out = std::env::temp_dir().join(format!("browserdeck-icon-{}.png", uuid::Uuid::new_v4()));
    let made = run(Command::new("sips").args(["-s", "format", "png", "-Z", "256"]).arg(&icns).arg("--out").arg(&out));
    let png = made.and_then(|_| std::fs::read(&out).ok());
    let _ = std::fs::remove_file(&out);
    png
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn png(_path: &Path) -> Option<Vec<u8>> {
    None
}

/// Cuts the transparent margin, so a small icon on a large canvas still fills the key.
/// None: nothing visible. An icon with no alpha at all (very old ones) counts as fully opaque.
#[cfg_attr(not(windows), allow(dead_code))]
fn trim(w: usize, h: usize, rgba: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let no_alpha = rgba.chunks_exact(4).all(|p| p[3] == 0);
    let solid = |x: usize, y: usize| no_alpha || rgba[(y * w + x) * 4 + 3] != 0;
    let cols: Vec<usize> = (0..w).filter(|&x| (0..h).any(|y| solid(x, y))).collect();
    let rows: Vec<usize> = (0..h).filter(|&y| (0..w).any(|x| solid(x, y))).collect();
    let (&x0, &x1, &y0, &y1) = (cols.first()?, cols.last()?, rows.first()?, rows.last()?);
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut out = Vec::with_capacity(cw * ch * 4);
    for y in y0..=y1 {
        out.extend_from_slice(&rgba[(y * w + x0) * 4..(y * w + x1 + 1) * 4]);
    }
    if no_alpha {
        out.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Some((cw, ch, out))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn encode(w: usize, h: usize, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(out)
}

/// The jumbo (256 px) icon from the system image list, as Explorer shows it; RGBA rows, top first.
#[cfg(windows)]
fn windows_icon(path: &Path) -> Option<(usize, usize, Vec<u8>)> {
    use windows::core::HSTRING;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX, SHIL_JUMBO};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let _com = crate::actions::ComGuard::init(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    let mut info = SHFILEINFOW::default();
    // SAFETY: `info` is a writable SHFILEINFOW and its size is the one passed.
    let found = unsafe {
        SHGetFileInfoW(
            &HSTRING::from(path.as_os_str()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX,
        )
    };
    if found == 0 {
        return None;
    }
    // SAFETY: COM is initialised on this thread; the icon handle is destroyed below.
    let icon = unsafe { SHGetImageList::<IImageList>(SHIL_JUMBO as i32).ok()?.GetIcon(info.iIcon, ILD_TRANSPARENT.0).ok()? };
    let mut ii = ICONINFO::default();
    // SAFETY: `icon` is a live icon; GetIconInfo hands us two bitmaps we delete below.
    let got = unsafe { GetIconInfo(icon, &mut ii) };
    // SAFETY: `icon` came from GetIcon and is not used after this.
    let _ = unsafe { DestroyIcon(icon) };
    got.ok()?;
    // SAFETY: reads `ii.hbmColor` into a buffer of exactly width * height * 4 bytes, then frees both bitmaps.
    unsafe {
        let mut bm = BITMAP::default();
        let read = GetObjectW(ii.hbmColor.into(), std::mem::size_of::<BITMAP>() as i32, Some((&mut bm as *mut BITMAP).cast()));
        let mut px = vec![0u8; (bm.bmWidth.max(0) * bm.bmHeight.max(0) * 4) as usize];
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bm.bmWidth,
                biHeight: -bm.bmHeight, // negative: top row first
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lines = if read == 0 || px.is_empty() {
            0
        } else {
            let dc = GetDC(None);
            let n = GetDIBits(dc, ii.hbmColor, 0, bm.bmHeight as u32, Some(px.as_mut_ptr().cast()), &mut bi, DIB_RGB_COLORS);
            ReleaseDC(None, dc);
            n
        };
        let _ = DeleteObject(ii.hbmColor.into());
        let _ = DeleteObject(ii.hbmMask.into());
        if lines == 0 {
            return None;
        }
        px.chunks_exact_mut(4).for_each(|p| p.swap(0, 2)); // BGRA to RGBA
        Some((bm.bmWidth as usize, bm.bmHeight as usize, px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(png: &[u8]) -> (u32, u32) {
        let info = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        (info.info().width, info.info().height)
    }

    #[test]
    fn the_transparent_margin_is_cut() {
        // 4 x 3, one visible pixel at (1, 1) and one at (2, 1)
        let mut px = vec![0u8; 4 * 3 * 4];
        for x in [1, 2] {
            px[(4 + x) * 4..(4 + x) * 4 + 4].copy_from_slice(&[10, 20, 30, 255]);
        }
        let (w, h, out) = trim(4, 3, &px).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, [10, 20, 30, 255, 10, 20, 30, 255]);
        assert_eq!(size(&encode(w, h, &out).unwrap()), (2, 1));
    }

    #[test]
    fn an_icon_without_alpha_is_kept_opaque_and_an_empty_one_is_none() {
        let (w, h, out) = trim(2, 2, &[9, 9, 9, 0].repeat(4)).unwrap();
        assert_eq!((w, h), (2, 2));
        assert!(out.chunks_exact(4).all(|p| p[3] == 255));
        assert!(trim(0, 0, &[]).is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_mac_app_gives_a_png() {
        let png = png(Path::new("/System/Applications/Calculator.app")).unwrap();
        assert!(size(&png).0 >= 128);
    }
}
