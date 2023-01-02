use egui::{vec2, Align2, NumExt, Pos2, Rect, Rounding, Sense, Stroke, TextStyle};

use crate::mechanics::Bar;

#[derive(Default)]
pub enum ProgressInfo {
    NextLevel {
        exp: usize,
    },
    Cubits {
        min: usize,
        max: usize,
    },
    Complete,
    #[default]
    Percent,
}

pub struct Progress<A = usize, B = usize> {
    pub pos: A,
    pub max: B,

    info: ProgressInfo,
}

pub trait ToF32 {
    fn as_f32(&self) -> f32;
}

impl ToF32 for usize {
    fn as_f32(&self) -> f32 {
        (*self) as _
    }
}
impl ToF32 for f32 {
    fn as_f32(&self) -> f32 {
        (*self) as _
    }
}

impl Progress<f32, f32> {
    pub const fn from_bar(Bar { max, pos }: Bar, info: ProgressInfo) -> Self {
        Self { pos, max, info }
    }
}

impl<A, B> Progress<A, B>
where
    A: ToF32,
    B: ToF32,
{
    pub fn display(self, ui: &mut egui::Ui) -> egui::Response {
        let row_height = ui
            .fonts()
            .row_height(&TextStyle::Monospace.resolve(ui.style()));

        let w = ui.available_size_before_wrap().x.at_least(96.0);
        let h = (ui.spacing().interact_size.y * 0.6).max(row_height);

        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        if !ui.is_rect_visible(resp.rect) {
            return resp;
        }

        let visuals = ui.style().visuals.clone();
        ui.painter()
            .rect(rect, Rounding::none(), visuals.window_fill, Stroke::NONE);

        let diff = self.pos.as_f32() / self.max.as_f32();

        ui.painter().rect(
            Rect::from_min_size(rect.min, vec2(rect.width() * diff, rect.height())),
            Rounding::none(),
            visuals.selection.bg_fill,
            Stroke::NONE,
        );

        let resp = resp.interact(Sense::hover());
        if resp.hovered() {
            use ProgressInfo::*;
            let overlay = match self.info {
                NextLevel { exp } => format!("{exp} exp required"),
                Cubits { min, max } => format!("{min}/{max} cubits"),
                Complete => {
                    let pct = self.pos.as_f32() / self.max.as_f32() * 100.0;
                    format!("{pct:.0}% complete")
                }
                Percent => {
                    let pct = self.pos.as_f32() / self.max.as_f32() * 100.0;
                    format!("{pct:.0}%")
                }
            };

            let fid = TextStyle::Monospace.resolve(ui.style());
            let (width, height) = {
                let fonts = &*ui.fonts();
                let width = overlay
                    .chars()
                    .fold(0.0, |a, c| a + fonts.glyph_width(&fid, c));
                (width, row_height)
            };

            let mid = (rect.width() - width) * 0.5;
            let left = rect.left_top().x + mid;

            ui.painter().text(
                Pos2 {
                    x: left + 2.0,
                    y: rect.left_top().y + (height / 2.0),
                },
                Align2::LEFT_CENTER,
                overlay,
                fid,
                ui.visuals().strong_text_color(),
            );
        }

        resp
    }
}
