use nih_plug_vizia::vizia::prelude::*;

use super::super::{AppScaffoldEvent, ScaffoldPage};
use super::super::theme::AppTheme;

pub(crate) fn build_nav_menu(cx: &mut Context) {
    let app_theme = AppTheme::new();

    VStack::new(cx, |cx| {
        Label::new(cx, "Sections").font_size(18.0);
        nav_button(cx, "Kick", ScaffoldPage::Kick, &app_theme);
        nav_button(cx, "Bass", ScaffoldPage::Bass, &app_theme);
        nav_button(cx, "Drum Machine", ScaffoldPage::DrumMachine, &app_theme);
        nav_button(cx, "Settings", ScaffoldPage::Settings, &app_theme);
        nav_button(cx, "Oscilloscope", ScaffoldPage::Oscilloscope, &app_theme);
        nav_button(cx, "Logs", ScaffoldPage::Logs, &app_theme);
        Label::new(cx, "Only app_scaffold is implemented.").font_size(12.0);
    })
    .class("app-nav")
    .height(Stretch(1.0));
}

fn nav_button(cx: &mut Context, label: &'static str, page: ScaffoldPage, app_theme: &AppTheme) {
    let button_bg = app_theme.button.background;
    let button_fg = app_theme.button.foreground;

    Button::new(
        cx,
        move |cx| cx.emit(AppScaffoldEvent::SelectPage(page)),
        move |cx| Label::new(cx, label).color(button_fg),
    )
    .class("page-button")
    .background_color(button_bg);
}
