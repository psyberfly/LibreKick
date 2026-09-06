#![allow(dead_code, non_snake_case)]

use nih_plug_vizia::vizia::prelude::Color;

#[derive(Debug, Clone)]
pub struct AppTheme {
    pub colors: Colors,
    pub button: ButtonTheme,
}

impl AppTheme {
    pub fn new() -> Self {
        let colors = Colors::new();
        let button = ButtonTheme {
            background: colors.primary,
            foreground: colors.onPrimary,
        };

        Self { colors, button }
    }
}

#[derive(Debug, Clone)]
pub struct ButtonTheme {
    pub background: Color,
    pub foreground: Color,
}

impl ButtonTheme {
    pub fn new() -> Self {
        Self {
            background: Colors::new().primary,
            foreground: Colors::new().onPrimary,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Colors {
    pub background: Color,
    pub primary: Color,
    pub onPrimary: Color,
    pub secondary: Color,
    pub onSecondary: Color,
    //pub components: ColorsByComponent,
}

impl Colors {
    pub fn new() -> Self {
        let darkGrey = Color::rgb(10, 12, 14);
        let orange = Color::rgb(245, 160, 88);
        let white = Color::rgb(255, 255, 255);
        let blue = Color::rgb(64, 120, 255);

        Self {
            background: darkGrey,
            primary: orange,
            onPrimary: white,
            secondary: blue,
            onSecondary: white,
        }
    }
}