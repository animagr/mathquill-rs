//! `RaTeX` rendering pipeline: LaTeX string → PNG bytes → egui texture.

use anyhow::{Context, Result};

use ratex_layout::{layout, to_display_list, LayoutBox, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::color::Color;
use ratex_types::display_item::DisplayList;

#[cfg(test)]
const LATEX_AUTO_SYMBOL_WITH_TYPED_SPACE: &str = "\\alpha \\,";

/// Render settings that affect the final PNG coordinate space.
#[derive(Debug, Clone, Copy)]
pub struct RenderMetrics {
    font_size: f32,
    padding: f32,
    device_pixel_ratio: f32,
}

impl RenderMetrics {
    /// Create render metrics from the options passed to `RaTeX`.
    #[must_use]
    pub fn from_options(options: &RenderOptions) -> Self {
        Self {
            font_size: options.font_size,
            padding: options.padding,
            device_pixel_ratio: options.device_pixel_ratio,
        }
    }

    /// Font size in user units per em.
    #[must_use]
    pub fn font_size(self) -> f32 {
        self.font_size
    }

    /// Output padding in user units.
    #[must_use]
    pub fn padding(self) -> f32 {
        self.padding
    }

    /// Output device-pixel ratio.
    #[must_use]
    pub fn device_pixel_ratio(self) -> f32 {
        self.device_pixel_ratio
    }
}

/// Rendered math output and the layout metadata used to produce it.
#[derive(Debug, Clone)]
pub struct RenderedMath {
    png_bytes: Vec<u8>,
    layout_box: LayoutBox,
    display_list: DisplayList,
    metrics: RenderMetrics,
}

impl RenderedMath {
    /// PNG bytes produced by the `RaTeX` renderer.
    #[must_use]
    pub fn png_bytes(&self) -> &[u8] {
        &self.png_bytes
    }

    /// Structured box tree produced by `RaTeX` layout.
    #[must_use]
    pub fn layout_box(&self) -> &LayoutBox {
        &self.layout_box
    }

    /// Flat drawing list produced from the layout box tree.
    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        &self.display_list
    }

    /// Render settings used to produce this output.
    #[must_use]
    pub fn metrics(&self) -> RenderMetrics {
        self.metrics
    }
}

/// Cached render state: avoids re-rendering when the LaTeX hasn't changed.
pub struct RenderCache {
    last_latex: String,
    last_rendered: Option<RenderedMath>,
    render_opts: RenderOptions,
    layout_opts: LayoutOptions,
}

impl RenderCache {
    /// Create a new render cache with default options.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_latex: String::new(),
            last_rendered: None,
            render_opts: RenderOptions {
                font_size: 40.0,
                padding: 10.0,
                font_dir: String::new(),
                device_pixel_ratio: 2.0,
                background_color: Color::WHITE,
            },
            layout_opts: LayoutOptions::default(),
        }
    }

    /// Render the given LaTeX string, returning PNG bytes and layout metadata.
    /// Returns cached result if the LaTeX hasn't changed.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing or rendering the LaTeX fails.
    pub fn render(&mut self, latex: &str) -> Result<&RenderedMath> {
        let cache_hit = latex == self.last_latex && self.last_rendered.is_some();
        if cache_hit {
            return self
                .last_rendered
                .as_ref()
                .context("render cache was marked as a hit but was empty");
        }

        let rendered = render_latex(latex, &self.layout_opts, &self.render_opts)?;
        self.last_latex = latex.to_string();
        self.last_rendered = Some(rendered);

        self.last_rendered
            .as_ref()
            .context("render cache was empty after rendering")
    }

    /// The most recently rendered math output, if any.
    #[must_use]
    pub fn last_rendered(&self) -> Option<&RenderedMath> {
        self.last_rendered.as_ref()
    }

    /// Set the font size (user units per em).
    pub fn set_font_size(&mut self, size: f32) {
        self.render_opts.font_size = size;
        self.last_latex.clear();
        self.last_rendered = None;
    }

    /// Set the device pixel ratio for high-DPI rendering.
    pub fn set_dpr(&mut self, dpr: f32) {
        self.render_opts.device_pixel_ratio = dpr;
        self.last_latex.clear();
        self.last_rendered = None;
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
) -> Result<RenderedMath> {
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

    Ok(RenderedMath {
        png_bytes: png,
        layout_box,
        display_list,
        metrics: RenderMetrics::from_options(render_opts),
    })
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
    use super::{RenderCache, LATEX_AUTO_SYMBOL_WITH_TYPED_SPACE};

    #[test]
    fn render_simple_expression() {
        let mut cache = RenderCache::new();
        let result = cache.render("x^2");
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let png = result.unwrap().png_bytes();
        assert!(!png.is_empty());
        // PNG magic bytes.
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn cache_returns_same_result() {
        let mut cache = RenderCache::new();
        let _ = cache.render("a+b").unwrap();
        let ptr1 = cache.render("a+b").unwrap().png_bytes().as_ptr();
        let ptr2 = cache.render("a+b").unwrap().png_bytes().as_ptr();
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn render_empty_latex() {
        let mut cache = RenderCache::new();
        let result = cache.render("");
        assert!(result.is_ok());
    }

    #[test]
    fn render_auto_symbol_followed_by_typed_space() {
        let mut cache = RenderCache::new();
        let result = cache.render(LATEX_AUTO_SYMBOL_WITH_TYPED_SPACE);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
    }

    #[test]
    fn render_exposes_layout_metadata() {
        let mut cache = RenderCache::new();
        let rendered = cache.render("x+1").unwrap();

        assert!(rendered.layout_box().width > 0.0);
        assert!(rendered.display_list().width > 0.0);
        assert!(!rendered.display_list().items.is_empty());
    }

    #[test]
    fn render_exposes_metrics() {
        let mut cache = RenderCache::new();
        let rendered = cache.render("x").unwrap();

        assert!(rendered.metrics().font_size() > 0.0);
        assert!(rendered.metrics().device_pixel_ratio() > 0.0);
    }
}
