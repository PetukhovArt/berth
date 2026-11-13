use tauri::{AppHandle, Manager, UserAttentionType};

const MAIN_WINDOW: &str = "main";

pub fn sync(app: &AppHandle, previous: u32, waiting: u32) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    show_count(&window, waiting);
    if waiting > previous && !window.is_focused().unwrap_or(false) {
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
    } else if waiting == 0 {
        let _ = window.request_user_attention(None);
    }
}

#[cfg(windows)]
fn show_count(window: &tauri::WebviewWindow, count: u32) {
    let icon = (count > 0).then(|| badge::render(count));
    let _ = window.set_overlay_icon(icon);
}

#[cfg(not(windows))]
fn show_count(window: &tauri::WebviewWindow, count: u32) {
    let _ = window.set_badge_count((count > 0).then_some(i64::from(count)));
}

#[cfg(windows)]
mod badge {
    use tauri::image::Image;

    const SIZE: usize = 32;
    const FILL: [u8; 3] = [0xd9, 0x30, 0x25];
    const GLYPHS: [[u8; 5]; 11] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b010, 0b010, 0b010],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
        [0b000, 0b010, 0b111, 0b010, 0b000],
    ];
    const PLUS: usize = 10;

    pub fn render(count: u32) -> Image<'static> {
        let mut rgba = vec![0u8; SIZE * SIZE * 4];
        let center = SIZE as f32 / 2.0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let coverage = (center - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
                let i = (y * SIZE + x) * 4;
                rgba[i..i + 3].copy_from_slice(&FILL);
                rgba[i + 3] = (coverage * 255.0) as u8;
            }
        }
        let glyphs: Vec<usize> = if count > 9 {
            vec![9, PLUS]
        } else {
            vec![count as usize]
        };
        let scale = if glyphs.len() == 1 { 4 } else { 3 };
        let width = (glyphs.len() * 4 - 1) * scale;
        let left = (SIZE - width) / 2;
        let top = (SIZE - 5 * scale) / 2;
        for (n, glyph) in glyphs.iter().enumerate() {
            for (row, bits) in GLYPHS[*glyph].iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) == 0 {
                        continue;
                    }
                    for py in 0..scale {
                        for px in 0..scale {
                            let x = left + (n * 4 + col) * scale + px;
                            let y = top + row * scale + py;
                            let i = (y * SIZE + x) * 4;
                            rgba[i..i + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
                        }
                    }
                }
            }
        }
        Image::new_owned(rgba, SIZE as u32, SIZE as u32)
    }
}
