//! `RaTeX` rendering pipeline: LaTeX string → PNG bytes → egui texture.

use anyhow::{Context, Result};

use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};

/// Cached render state: avoids re-rendering when the LaTeX hasn't changed.
pub struct RenderCache {
    last_latex: String,
    last_png: Vec<u8>,
    render_opts: RenderOptions,
    layout_opts: LayoutOptions,
}

impl RenderCache {
    /// Create a new render cache with default options.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_latex: String::new(),
            last_png: Vec::new(),
            render_opts: RenderOptions {
                font_size: 40.0,
                padding: 10.0,
                font_dir: String::new(),
                device_pixel_ratio: 2.0,
            },
            layout_opts: LayoutOptions::default(),
        }
    }

    /// Render the given LaTeX string, returning PNG bytes.
    /// Returns cached result if the LaTeX hasn't changed.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing or rendering the LaTeX fails.
    pub fn render(&mut self, latex: &str) -> Result<&[u8]> {
        if latex == self.last_latex && !self.last_png.is_empty() {
            return Ok(&self.last_png);
        }

        let png = render_latex(latex, &self.layout_opts, &self.render_opts)?;
        self.last_latex = latex.to_string();
        self.last_png = png;
        Ok(&self.last_png)
    }

    /// Set the font size (user units per em).
    pub fn set_font_size(&mut self, size: f32) {
        self.render_opts.font_size = size;
        self.last_latex.clear();
    }

    /// Set the device pixel ratio for high-DPI rendering.
    pub fn set_dpr(&mut self, dpr: f32) {
        self.render_opts.device_pixel_ratio = dpr;
        self.last_latex.clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a LaTeX string to PNG bytes using the full `RaTeX` pipeline.
fn render_latex(
    latex: &str,
    layout_opts: &LayoutOptions,
    render_opts: &RenderOptions,
) -> Result<Vec<u8>> {
    if latex.is_empty() {
        return render_latex("{}", layout_opts, render_opts);
    }

    let ast = parse(latex)
        .map_err(|e| anyhow::anyhow!("RaTeX parse error: {e}"))
        .context("Failed to parse LaTeX")?;

    let layout_box = layout(&ast, layout_opts);
    let display_list = to_display_list(&layout_box);

    let png = render_to_png(&display_list, render_opts)
        .map_err(|e| anyhow::anyhow!("RaTeX render error: {e}"))
        .context("Failed to render to PNG")?;

    Ok(png)
}

/// Load PNG bytes into an egui `ColorImage`.
///
/// # Errors
///
/// Returns an error if the PNG bytes cannot be decoded.
pub fn png_to_color_image(png_bytes: &[u8]) -> Result<egui::ColorImage> {
    let img = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .context("Failed to decode PNG")?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels: Vec<egui::Color32> = rgba
        .pixels()
        .map(|p| egui::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    Ok(egui::ColorImage { size, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_expression() {
        let mut cache = RenderCache::new();
        let result = cache.render("x^2");
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let png = result.unwrap();
        assert!(!png.is_empty());
        // PNG magic bytes.
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn cache_returns_same_result() {
        let mut cache = RenderCache::new();
        let _ = cache.render("a+b").unwrap();
        let ptr1 = cache.render("a+b").unwrap().as_ptr();
        let ptr2 = cache.render("a+b").unwrap().as_ptr();
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn render_empty_latex() {
        let mut cache = RenderCache::new();
        let result = cache.render("");
        assert!(result.is_ok());
    }
}
