use crate::{
    browser::{BrowserId, BrowserSnapshot, CurrentBrowser},
    config::{Config, SwitchMode, ThemePreference},
    switching::SwitchResult,
};
use anyhow::{Context, Result};
use fontdue::{
    layout::{CoordinateSystem, Layout, TextStyle},
    Font,
};
use std::sync::OnceLock;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Transform};
use tray_icon::Icon;

pub const WIDTH: u32 = 169;
pub const HEIGHT: u32 = 120;
pub const BUTTON: (f32, f32, f32, f32) = (10.0, 78.0, 149.0, 30.0);

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        let windows = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        ["segoeui.ttf", "arial.ttf"].into_iter().find_map(|name| {
            std::fs::read(std::path::PathBuf::from(&windows).join("Fonts").join(name))
                .ok()
                .and_then(|bytes| Font::from_bytes(bytes, fontdue::FontSettings::default()).ok())
        })
    })
    .as_ref()
}

fn rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    if Rect::from_xywh(x, y, w, h).is_none() {
        return;
    }
    let radius = radius.clamp(0.0, w.min(h) / 2.0);
    let mut builder = PathBuilder::new();
    builder.move_to(x + radius, y);
    builder.line_to(x + w - radius, y);
    builder.quad_to(x + w, y, x + w, y + radius);
    builder.line_to(x + w, y + h - radius);
    builder.quad_to(x + w, y + h, x + w - radius, y + h);
    builder.line_to(x + radius, y + h);
    builder.quad_to(x, y + h, x, y + h - radius);
    builder.line_to(x, y + radius);
    builder.quad_to(x, y, x + radius, y);
    builder.close();
    let Some(path) = builder.finish() else { return };
    let mut paint = Paint::default();
    paint.set_color(color);
    pm.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn text_width(value: &str, size: f32) -> f32 {
    font()
        .map(|font| {
            value
                .chars()
                .map(|character| font.metrics(character, size).advance_width)
                .sum()
        })
        .unwrap_or(0.0)
}

fn centered_text(pm: &mut Pixmap, value: &str, y: f32, size: f32, color: Color) {
    let x = ((pm.width() as f32 - text_width(value, size)) / 2.0).max(0.0);
    text(pm, value, x, y, size, color);
}

fn text(pm: &mut Pixmap, value: &str, x: f32, y: f32, size: f32, color: Color) {
    let Some(font) = font() else { return };
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&fontdue::layout::LayoutSettings {
        x,
        y,
        ..Default::default()
    });
    layout.append(&[font], &TextStyle::new(value, size, 0));
    let width = pm.width() as i32;
    let height = pm.height() as i32;
    let (cr, cg, cb, ca) = (
        (color.red() * 255.0) as u32,
        (color.green() * 255.0) as u32,
        (color.blue() * 255.0) as u32,
        (color.alpha() * 255.0) as u32,
    );
    let data = pm.data_mut();
    for glyph in layout.glyphs() {
        let (metrics, bitmap) = font.rasterize_config(glyph.key);
        for (i, coverage) in bitmap.into_iter().enumerate() {
            if coverage == 0 {
                continue;
            }
            let px = glyph.x as i32 + (i % metrics.width) as i32;
            let py = glyph.y as i32 + (i / metrics.width) as i32;
            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }
            let index = ((py * width + px) * 4) as usize;
            let alpha = coverage as u32 * ca / 255;
            let inverse = 255 - alpha;
            data[index] = ((cr * alpha + data[index] as u32 * inverse) / 255) as u8;
            data[index + 1] = ((cg * alpha + data[index + 1] as u32 * inverse) / 255) as u8;
            data[index + 2] = ((cb * alpha + data[index + 2] as u32 * inverse) / 255) as u8;
            data[index + 3] = (alpha + data[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

fn is_light(config: &Config) -> bool {
    match config.theme {
        ThemePreference::Light => true,
        ThemePreference::Dark => false,
        ThemePreference::System => tao::window::Theme::default() == tao::window::Theme::Light,
    }
}

pub fn render(
    snapshot: &BrowserSnapshot,
    config: &Config,
    feedback: Option<&SwitchResult>,
) -> Result<Pixmap> {
    let mut pm = Pixmap::new(WIDTH, HEIGHT).context("allocate widget pixels")?;
    let light = is_light(config);
    let alpha = (config.opacity * 255.0) as u8;
    let background = if light {
        rgba(246, 248, 252, alpha)
    } else {
        rgba(16, 20, 29, alpha)
    };
    let foreground = if light {
        rgba(25, 31, 43, 255)
    } else {
        rgba(241, 245, 249, 255)
    };
    let muted = if light {
        rgba(91, 101, 117, 255)
    } else {
        rgba(157, 168, 185, 255)
    };
    rounded_rect(
        &mut pm,
        0.0,
        0.0,
        WIDTH as f32,
        HEIGHT as f32,
        10.0,
        background,
    );
    text(
        &mut pm,
        "QUICK BROWSER SWITCHER",
        10.0,
        9.0,
        8.0,
        rgba(64, 126, 255, 255),
    );
    text(
        &mut pm,
        snapshot.current.label(),
        10.0,
        27.0,
        17.0,
        foreground,
    );
    let status = feedback
        .map(|f| f.message.as_str())
        .unwrap_or_else(|| match snapshot.current {
            CurrentBrowser::Mixed => "Web associations do not all match",
            CurrentBrowser::Unknown => "Choose a browser to configure Windows",
            _ => match config.switch_mode {
                SwitchMode::Supported => "Supported confirmation mode",
                SwitchMode::Experimental => "Experimental silent mode",
            },
        });
    let status = if status.chars().count() > 27 {
        format!("{}…", status.chars().take(26).collect::<String>())
    } else {
        status.to_string()
    };
    text(&mut pm, &status, 10.0, 52.0, 9.0, muted);
    let target = snapshot.target();
    let available = target
        .and_then(|id| snapshot.browser(id))
        .is_some_and(|b| b.installed);
    let button_color = if available {
        if light {
            rgba(63, 63, 70, 255)
        } else {
            rgba(82, 82, 91, 255)
        }
    } else {
        rgba(113, 113, 122, 180)
    };
    rounded_rect(
        &mut pm,
        BUTTON.0,
        BUTTON.1,
        BUTTON.2,
        BUTTON.3,
        8.0,
        button_color,
    );
    let label = match target {
        Some(BrowserId::Chrome) => "Switch to Chrome",
        Some(BrowserId::Edge) => "Switch to Edge",
        None => "Browser unavailable",
    };
    centered_text(&mut pm, label, 85.0, 12.0, rgba(255, 255, 255, 255));
    Ok(pm)
}

pub fn hit_button(x: f64, y: f64) -> bool {
    x >= BUTTON.0 as f64
        && x <= (BUTTON.0 + BUTTON.2) as f64
        && y >= BUTTON.1 as f64
        && y <= (BUTTON.1 + BUTTON.3) as f64
}

pub fn tray_icon(current: &CurrentBrowser) -> Result<Icon> {
    let size = 32u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let color = match current {
        CurrentBrowser::Chrome => [66, 133, 244],
        CurrentBrowser::Edge => [10, 167, 157],
        CurrentBrowser::Mixed => [245, 158, 11],
        _ => [125, 135, 150],
    };
    for y in 2..30 {
        for x in 2..30 {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            if dx * dx + dy * dy <= 14 * 14 {
                let i = ((y * size + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    Icon::from_rgba(data, size, size).map_err(|e| anyhow::anyhow!("tray icon: {e}"))
}

pub fn pixmap_to_softbuffer(pm: &Pixmap, target: &mut [u32]) {
    for (out, rgba) in target.iter_mut().zip(pm.data().chunks_exact(4)) {
        *out = (rgba[0] as u32) << 16 | (rgba[1] as u32) << 8 | rgba[2] as u32;
    }
}
