use std::f64::consts::PI;

use ratatui::prelude::*;
use ratatui::widgets::canvas::{Circle, Line, *};

use crate::ui::Palette;

#[derive(Default)]
pub struct FlyVisualWidget {}

fn draw_balloon(ctx: &mut Context, x: f64, y: f64, size: f64, color: Color) {
    // Balloon
    ctx.draw(&Circle {
        x,
        y,
        radius: size,
        color,
    });

    // Basket
    let basket_width = size * 0.6;
    let basket_height = size * 0.4;
    let basket_bottom = y - size * 2.2;
    ctx.draw(&Line {
        x1: x - basket_width / 2.0,
        y1: basket_bottom,
        x2: x + basket_width / 2.0,
        y2: basket_bottom,
        color,
    });
    ctx.draw(&Line {
        x1: x - basket_width / 2.0,
        y1: basket_bottom + basket_height,
        x2: x + basket_width / 2.0,
        y2: basket_bottom + basket_height,
        color,
    });
    ctx.draw(&Line {
        x1: x - basket_width / 2.0,
        y1: basket_bottom,
        x2: x - basket_width / 2.0,
        y2: basket_bottom + basket_height,
        color,
    });
    ctx.draw(&Line {
        x1: x + basket_width / 2.0,
        y1: basket_bottom,
        x2: x + basket_width / 2.0,
        y2: basket_bottom + basket_height,
        color,
    });

    // Ropes
    ctx.draw(&Line {
        x1: x - basket_width / 2.0,
        y1: y + size * 0.8,
        x2: x - basket_width / 2.0,
        y2: basket_bottom + basket_height,
        color,
    });
    ctx.draw(&Line {
        x1: x + basket_width / 2.0,
        y1: y + size * 0.8,
        x2: x + basket_width / 2.0,
        y2: basket_bottom + basket_height,
        color,
    });
}

fn generate_world_points(cx: f64, cy: f64, a: f64, b: f64, color: Color) -> Vec<(f64, f64, Color)> {
    let mut points = Vec::new();
    for r in 0..=100 {
        let r = r as f64 / 100.0;
        for t in 0..360 {
            let angle = t as f64 * PI / 180.0;
            let x = cx + a * r * angle.cos();
            let y = cy + b * r * angle.sin();
            points.push((x, y, color));
        }
    }
    points
}

fn generate_cloud_points(cx: f64, cy: f64, size: f64) -> Vec<(f64, f64, Color)> {
    let mut points = Vec::new();
    for r in 0..=100 {
        let r = r as f64 / 100.0;
        for t in 0..360 {
            let angle = t as f64 * PI / 180.0;
            points.push((
                cx + size * r * angle.cos(),
                cy + size * 0.6 * r * angle.sin(),
                Color::White,
            ));
            points.push((
                cx - size * 0.6 + size * 0.7 * r * angle.cos(),
                cy + size * 0.2 + size * 0.4 * r * angle.sin(),
                Color::White,
            ));
            points.push((
                cx + size * 0.6 + size * 0.7 * r * angle.cos(),
                cy + size * 0.2 + size * 0.4 * r * angle.sin(),
                Color::White,
            ));
        }
    }
    points
}

impl Widget for FlyVisualWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let canvas = Canvas::default()
            .x_bounds([-1.0, 1.0])
            .y_bounds([-1.0, 1.0])
            .background_color(Color::Black)
            .paint(|ctx| {
                let mut points = Vec::new();

                // World
                let globe_color = Palette::LIGHT_PURPLE;
                points.extend(generate_world_points(0.0, 0.0, 0.8, 0.7, globe_color));

                // Clouds
                points.extend(generate_cloud_points(-0.9, 0.7, 0.25));
                points.extend(generate_cloud_points(0.9, 0.8, 0.3));
                points.extend(generate_cloud_points(-0.7, -0.9, 0.27));
                points.extend(generate_cloud_points(0.8, -0.8, 0.23));

                for (x, y, color) in points {
                    ctx.draw(&Points {
                        coords: &[(x, y)],
                        color,
                    });
                }

                // Balloons
                let balloon_colors = [
                    Palette::LIGHT_TEAL,
                    Palette::LIGHT_TEAL,
                    Palette::BLUE,
                    Palette::LIGHT_PINK,
                    Palette::DARK_BLUE,
                ];

                let balloon_positions = [
                    (-0.6, -0.3, 0.15),
                    (-0.2, -0.5, 0.12),
                    (0.5, -0.2, 0.18),
                    (-0.4, 0.5, 0.1),
                    (0.3, 0.6, 0.13),
                ];

                for (i, &(x, y, size)) in balloon_positions.iter().enumerate() {
                    let color = balloon_colors[i % balloon_colors.len()];
                    draw_balloon(ctx, x, y, size, color);
                }

                // Draw star icons
                let star_positions = [
                    (-0.95, 0.3),
                    (0.95, 0.4),
                    (-0.2, 0.95),
                    (0.1, -0.97),
                    (-0.8, -0.6),
                    (0.85, -0.5),
                    (-0.5, 0.8),
                    (0.6, 0.7),
                    (-0.7, 0.2),
                    (0.8, 0.3),
                ];
                for &(x, y) in &star_positions {
                    ctx.print(x, y, Span::styled("★", Style::default().fg(Color::Yellow)));
                }
            });

        canvas.render(area, buf);
    }
}
