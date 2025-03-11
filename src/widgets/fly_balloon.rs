use ratatui::prelude::*;
use ratatui::widgets::canvas::{Canvas, Context, Line};
use ratatui::widgets::Widget;

use crate::ui::Palette;

pub struct FlyBalloonWidget {
    primary_color: Color,
    secondary_color: Color,
}

impl FlyBalloonWidget {
    pub fn colors(mut self, primary: Color, secondary: Color) -> Self {
        self.primary_color = primary;
        self.secondary_color = secondary;
        self
    }

    fn draw_balloon(&self, ctx: &mut Context) {
        let balloon_points: Vec<(f64, f64)> = vec![
            (50.0, 10.0), // Top point
            (40.0, 15.0), // Smoothing top curve
            (32.0, 25.0), // Smoothing top curve
            (26.0, 40.0), // Left upper curve
            (23.0, 55.0), // Left middle
            (24.0, 70.0), // Left lower curve
            (28.0, 80.0), // Left bottom curve
            (35.0, 87.0), // Bottom left
            (50.0, 90.0), // Bottom point
            (65.0, 87.0), // Bottom right
            (72.0, 80.0), // Right bottom curve
            (76.0, 70.0), // Right lower curve
            (77.0, 55.0), // Right middle
            (74.0, 40.0), // Right upper curve
            (68.0, 25.0), // Smoothing top curve
            (60.0, 15.0), // Smoothing top curve
            (50.0, 10.0), // Back to top
        ]
        .into_iter()
        .map(|(x, y)| (x, 100.0 - y))
        .collect();

        // Fill balloon
        for y in 10..91 {
            let t = (y - 10) as f64 / 80.0;
            let color = interpolate_color(self.primary_color, self.secondary_color, t);
            let intersections = find_x_intersections(&balloon_points, 100.0 - y as f64);
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    ctx.draw(&Line {
                        x1: pair[0],
                        y1: y as f64,
                        x2: pair[1],
                        y2: y as f64,
                        color,
                    });
                }
            }
        }

        // Basket
        let basket_points: Vec<(f64, f64)> = vec![
            (50.0, 90.0),
            (45.0, 92.0),
            (43.0, 95.0),
            (45.0, 98.0),
            (50.0, 99.0),
            (55.0, 98.0),
            (57.0, 95.0),
            (55.0, 92.0),
            (50.0, 90.0),
        ]
        .into_iter()
        .map(|(x, y)| (x, 100.0 - y))
        .collect();

        // Draw basket outline
        for i in 0..basket_points.len() - 1 {
            ctx.draw(&Line {
                x1: basket_points[i].0,
                y1: basket_points[i].1,
                x2: basket_points[i + 1].0,
                y2: basket_points[i + 1].1,
                color: self.secondary_color,
            });
        }
    }
}

impl Default for FlyBalloonWidget {
    fn default() -> Self {
        Self {
            primary_color: Palette::LIGHT_BLUE,
            secondary_color: Palette::LIGHT_PURPLE,
        }
    }
}

impl Widget for FlyBalloonWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Canvas::default()
            .paint(|ctx| {
                self.draw_balloon(ctx);
            })
            .x_bounds([0.0, 100.0])
            .y_bounds([0.0, 100.0])
            .background_color(Color::Black)
            .render(area, buf);
    }
}

fn interpolate_color(color1: Color, color2: Color, t: f64) -> Color {
    match (color1, color2) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let r = (r1 as f64 * (1.0 - t) + r2 as f64 * t) as u8;
            let g = (g1 as f64 * (1.0 - t) + g2 as f64 * t) as u8;
            let b = (b1 as f64 * (1.0 - t) + b2 as f64 * t) as u8;
            Color::Rgb(r, g, b)
        }
        _ => color1,
    }
}

fn find_x_intersections(points: &[(f64, f64)], y: f64) -> Vec<f64> {
    let mut intersections = Vec::new();
    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        if (y1 <= y && y <= y2) || (y2 <= y && y <= y1) {
            if y1 == y2 {
                intersections.push(x1.min(x2));
                intersections.push(x1.max(x2));
            } else {
                let t = (y - y1) / (y2 - y1);
                let x = x1 + t * (x2 - x1);
                intersections.push(x);
            }
        }
    }
    intersections.sort_by(|a, b| a.partial_cmp(b).unwrap());
    intersections
}
