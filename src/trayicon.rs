//! The app icon, drawn in code: a rounded tile with a blue→violet gradient and a white monitor.
//!
//! Shapes are signed distance fields in a 32×32 design space, so the icon is anti-aliased and sharp
//! at every size and DPI without shipping bitmaps. The same renderer writes `build/windows/icon.ico`
//! (`make icon`), which `build.rs` embeds as the executable's icon for Explorer.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;

use crate::win32::*;

/// Sizes written to the .ico: the shell's small/large icons at 100–200% scaling, and 128 for the big views
/// (a 256 DIB would add ~260 KB to the exe for the rarely used jumbo view).
pub const ICO_SIZES: [i32; 8] = [16, 20, 24, 32, 40, 48, 64, 128];

/// Signed distance from a rounded rectangle (centre cx,cy, half sides hw,hh, corner radius r).
fn sd_rrect(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - hw + r;
    let qy = (y - cy).abs() - hh + r;
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// ARGB pixels (straight alpha), top-down, of the icon at n×n.
pub fn render(n: i32) -> Vec<u32> {
    let k = n as f32 / 32.0; // pixels per design unit
    let cov = |d: f32| (0.5 - d * k).clamp(0.0, 1.0);
    let top = [79.0f32, 140.0, 255.0]; // #4F8CFF
    let bot = [124.0f32, 58.0, 237.0]; // #7C3AED
    let white = [255.0f32, 255.0, 255.0];
    let mut out = vec![0u32; (n * n) as usize];
    for py in 0..n {
        for px in 0..n {
            let x = (px as f32 + 0.5) / k;
            let y = (py as f32 + 0.5) / k;
            let mut c = [0.0f32; 4]; // premultiplied rgb + alpha
            let mut over = |col: [f32; 3], a: f32| {
                if a <= 0.0 {
                    return;
                }
                for i in 0..3 {
                    c[i] = col[i] * a + c[i] * (1.0 - a);
                }
                c[3] = a + c[3] * (1.0 - a);
            };
            // tile
            let t = ((x + y) / 64.0).clamp(0.0, 1.0);
            let grad = [top[0] + (bot[0] - top[0]) * t, top[1] + (bot[1] - top[1]) * t, top[2] + (bot[2] - top[2]) * t];
            over(grad, cov(sd_rrect(x, y, 16.0, 16.0, 15.5, 15.5, 7.5)));
            // monitor: frame, glass, neck, foot
            let body = sd_rrect(x, y, 16.0, 14.0, 9.5, 6.75, 2.4);
            over(white, 0.16 * cov(body + 1.15));
            over(white, cov(body.abs() - 1.15));
            over(white, cov(sd_rrect(x, y, 16.0, 22.4, 1.3, 1.7, 0.3)));
            over(white, cov(sd_rrect(x, y, 16.0, 24.5, 5.0, 1.15, 1.15)));
            let a = c[3];
            if a > 0.0 {
                let ch = |v: f32| ((v / a).round().clamp(0.0, 255.0)) as u32;
                out[(py * n + px) as usize] =
                    ((a * 255.0).round() as u32) << 24 | ch(c[0]) << 16 | ch(c[1]) << 8 | ch(c[2]);
            }
        }
    }
    out
}

/// An `HICON` of n×n pixels (no .ico file or resource needed at runtime).
pub fn make_icon(n: i32) -> isize {
    let n = n.max(16);
    // SAFETY: the DIB section is n×n×32bpp, exactly what the copied slice covers.
    unsafe {
        let mut bi: BITMAPINFO = zeroed();
        bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = n;
        bi.bmiHeader.biHeight = -n; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut c_void = null_mut();
        let color = CreateDIBSection(0, &bi, 0, &mut bits, 0, 0);
        if color == 0 || bits.is_null() {
            return 0;
        }
        let px = std::slice::from_raw_parts_mut(bits as *mut u32, (n * n) as usize);
        px.copy_from_slice(&render(n));
        let mask_bytes = vec![0u8; (((n + 15) / 16) * 2 * n) as usize];
        let mask = CreateBitmap(n, n, 1, 1, mask_bytes.as_ptr() as *const c_void);
        let ii = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}

/// A multi-size .ico with 32-bit DIB images (alpha channel, empty AND mask).
pub fn ico_bytes(sizes: &[i32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&n| {
            let mask_row = (((n + 31) / 32) * 4) as usize;
            let mut img = Vec::new();
            img.extend_from_slice(&40u32.to_le_bytes());
            img.extend_from_slice(&n.to_le_bytes());
            img.extend_from_slice(&(n * 2).to_le_bytes()); // XOR + AND bitmaps
            img.extend_from_slice(&1u16.to_le_bytes());
            img.extend_from_slice(&32u16.to_le_bytes());
            img.extend_from_slice(&[0; 24]);
            let px = render(n);
            for y in (0..n).rev() {
                for x in 0..n {
                    img.extend_from_slice(&px[(y * n + x) as usize].to_le_bytes());
                    // B G R A
                }
            }
            img.resize(img.len() + mask_row * n as usize, 0);
            img
        })
        .collect();
    let mut f = Vec::new();
    f.extend_from_slice(&[0, 0, 1, 0]);
    f.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&n, img) in sizes.iter().zip(&images) {
        let side = if n >= 256 { 0 } else { n as u8 }; // 0 means 256
        f.extend_from_slice(&[side, side, 0, 0]);
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&32u16.to_le_bytes());
        f.extend_from_slice(&(img.len() as u32).to_le_bytes());
        f.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        f.extend_from_slice(&img);
    }
    f
}

/// A 32-bit BMP showing the icon at several sizes on a light and a dark background (design check).
pub fn preview_bmp() -> Vec<u8> {
    let sizes = [16, 20, 24, 32, 48, 64, 128];
    let pad = 16;
    let w: i32 = sizes.iter().sum::<i32>() + pad * (sizes.len() as i32 + 1);
    let h = (128 + pad * 2) * 2;
    let mut img = vec![0u32; (w * h) as usize];
    for (row, bg) in [(0, 0xF3u32), (1, 0x20u32)] {
        let y0 = row * (128 + pad * 2);
        for y in y0..y0 + 128 + pad * 2 {
            for x in 0..w {
                img[(y * w + x) as usize] = bg << 16 | bg << 8 | bg;
            }
        }
        let mut x0 = pad;
        for &n in &sizes {
            let px = render(n);
            let oy = y0 + pad + (128 - n) / 2;
            for y in 0..n {
                for x in 0..n {
                    let p = px[(y * n + x) as usize];
                    let a = (p >> 24) as f32 / 255.0;
                    let d = &mut img[((oy + y) * w + x0 + x) as usize];
                    let mix =
                        |s: u32, sh: u32| ((((p >> sh) & 0xFF) as f32 * a + s as f32 * (1.0 - a)).round() as u32) << sh;
                    *d = mix(bg, 16) | mix(bg, 8) | mix(bg, 0);
                }
            }
            x0 += n + pad;
        }
    }
    let size = 54 + img.len() as u32 * 4;
    let mut f = Vec::with_capacity(size as usize);
    f.extend_from_slice(b"BM");
    f.extend_from_slice(&size.to_le_bytes());
    f.extend_from_slice(&[0; 4]);
    f.extend_from_slice(&54u32.to_le_bytes());
    f.extend_from_slice(&40u32.to_le_bytes());
    f.extend_from_slice(&w.to_le_bytes());
    f.extend_from_slice(&(-h).to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&32u16.to_le_bytes());
    f.extend_from_slice(&[0; 24]);
    for p in img {
        f.extend_from_slice(&(p | 0xFF00_0000).to_le_bytes());
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_transparent_and_centre_is_opaque() {
        for n in [16, 32, 256] {
            let px = render(n);
            assert_eq!(px.len(), (n * n) as usize);
            assert_eq!(px[0] >> 24, 0, "top-left corner at {n}");
            assert_eq!(px[(n * n - 1) as usize] >> 24, 0, "bottom-right corner at {n}");
            assert_eq!(px[(n / 2 * n + n / 2) as usize] >> 24, 0xFF, "centre at {n}");
        }
    }

    #[test]
    fn ico_directory_is_consistent() {
        let sizes = [16, 256];
        let f = ico_bytes(&sizes);
        assert_eq!(&f[..6], &[0, 0, 1, 0, 2, 0]);
        let mut end = 6 + 16 * sizes.len();
        for (i, &n) in sizes.iter().enumerate() {
            let e = &f[6 + 16 * i..6 + 16 * (i + 1)];
            assert_eq!(e[0], if n == 256 { 0 } else { n as u8 });
            let len = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
            let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
            assert_eq!(off, end, "image {i} follows the previous one");
            let mask = (n as usize).div_ceil(32) * 4 * n as usize;
            assert_eq!(len, 40 + (n * n * 4) as usize + mask);
            end = off + len;
        }
        assert_eq!(end, f.len());
    }
}
