use nih_plug_egui::egui::{
    self, Align2, ColorImage, Pos2, Sense, TextureHandle, TextureOptions, Vec2,
};

use crate::ui::theme::{themed_font, APP_THEME};

pub(crate) fn brand_logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let image_bytes = include_bytes!("../../assets/logo.png");
    let image = image::load_from_memory_with_format(image_bytes, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let [width, height] = [image.width() as usize, image.height() as usize];
    let pixels = image.into_raw();
    let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);

    Some(ctx.load_texture(
        "librekick-brand-logo",
        color_image,
        TextureOptions::LINEAR,
    ))
}

fn title_logo_size(ui: &egui::Ui, scale: f32) -> Vec2 {
    let font = themed_font((26.0 * scale).max(18.0));
    let text_size = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap("LibreKick".to_owned(), font, APP_THEME.brand_orange())
            .size()
    });
    text_size + Vec2::new(10.0 * scale, 6.0 * scale)
}

pub(crate) fn brand_title_logo(ui: &mut egui::Ui, logo: Option<&TextureHandle>, scale: f32) {
    let target_size = title_logo_size(ui, scale);

    if let Some(logo) = logo {
        let image_size = logo.size_vec2();
        if image_size.x > 0.0 && image_size.y > 0.0 {
            let fit = (target_size.x / image_size.x).min(target_size.y / image_size.y);
            let draw_size = image_size * fit;
            ui.add(egui::Image::new(logo).fit_to_exact_size(draw_size));
            return;
        }
    }

    glowing_brand_label(ui, "LibreKick", scale);
}

fn glowing_brand_label(ui: &mut egui::Ui, text: &str, scale: f32) {
    let font = themed_font((26.0 * scale).max(18.0));
    let text_size = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font.clone(), APP_THEME.brand_orange())
            .size()
    });
    let padding = Vec2::new(10.0 * scale, 6.0 * scale);
    let (rect, _) = ui.allocate_exact_size(text_size + padding, Sense::hover());
    let text_pos = Pos2::new(rect.left() + 4.0 * scale, rect.center().y - text_size.y * 0.5);
    let painter = ui.painter();

    let outer_offsets = [
        Vec2::new(-3.0, 0.0),
        Vec2::new(3.0, 0.0),
        Vec2::new(0.0, -3.0),
        Vec2::new(0.0, 3.0),
        Vec2::new(-2.0, -2.0),
        Vec2::new(2.0, -2.0),
        Vec2::new(-2.0, 2.0),
        Vec2::new(2.0, 2.0),
    ];
    for offset in outer_offsets {
        painter.text(
            text_pos + offset * scale,
            Align2::LEFT_TOP,
            text,
            font.clone(),
            APP_THEME.brand_glow_outer(),
        );
    }

    let inner_offsets = [
        Vec2::new(-1.2, 0.0),
        Vec2::new(1.2, 0.0),
        Vec2::new(0.0, -1.2),
        Vec2::new(0.0, 1.2),
    ];
    for offset in inner_offsets {
        painter.text(
            text_pos + offset * scale,
            Align2::LEFT_TOP,
            text,
            font.clone(),
            APP_THEME.brand_glow_inner(),
        );
    }

    painter.text(
        text_pos,
        Align2::LEFT_TOP,
        text,
        font.clone(),
        APP_THEME.brand_hot_core(),
    );
    painter.text(
        text_pos + Vec2::new(0.0, -0.2 * scale),
        Align2::LEFT_TOP,
        text,
        font,
        APP_THEME.brand_orange(),
    );
}
