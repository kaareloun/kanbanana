//! The card that follows the pointer while you drag it, and while it settles into its slot.

use eframe::egui::{
    Context, CornerRadius, Id, LayerId, Mesh, Order, Rect, Shadow, Shape,
    emath::{Rot2, TSTransform},
    epaint::Tessellator,
    layers::ShapeIdx,
};

use super::card::{self, CardData, Fx};
use crate::theme::Theme;

/// Paints the card on top of everything, scaled around its center and turned by `angle` radians.
/// `lift` from 0 to 1 sets the shadow strength.
/// egui transforms only scale and move, so the card is tessellated here and the mesh is turned.
#[allow(clippy::too_many_arguments)]
pub fn paint_floating(ctx: &Context, t: &Theme, rect: Rect, c: &CardData, angle: f32, scale: f32, lift: f32, fx: Fx) {
    let layer = LayerId::new(Order::Tooltip, Id::unique("lb-drag-card"));
    let painter = ctx.layer_painter(layer);
    let start = ctx.graphics(|g| g.get(layer).map_or(0, |l| l.next_idx().0));
    let a = (60.0 + 100.0 * lift) as u8;
    let shadow = Shadow { offset: [0, (4.0 + 14.0 * lift) as i8], blur: (10.0 + 26.0 * lift) as u8, spread: 0, color: t.shade(a) };
    painter.add(shadow.as_shape(rect, CornerRadius::same(card::RADIUS)));
    card::paint(&painter, t, rect, c, fx);

    let ppp = ctx.pixels_per_point();
    let options = ctx.tessellation_options(|o| *o);
    let tex_size = ctx.fonts(|f| f.font_image_size());
    let clip = ctx.content_rect();
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        let end = list.next_idx().0;
        let mut shapes = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            list.mutate_shape(ShapeIdx(i), |cs| shapes.push(std::mem::replace(&mut cs.shape, Shape::Noop)));
        }
        // One mesh per texture: the avatar photo has its own texture, text and fills share one.
        let mut tess = Tessellator::new(ppp, options, tex_size, Vec::new());
        let mut meshes: Vec<Mesh> = Vec::new();
        for s in shapes {
            let mut m = Mesh::default();
            tess.tessellate_shape(s, &mut m);
            match meshes.last_mut() {
                Some(last) if last.texture_id == m.texture_id => last.append(m),
                _ if m.is_empty() => {}
                _ => meshes.push(m),
            }
        }
        let c = rect.center().to_vec2();
        for mut mesh in meshes {
            mesh.transform(TSTransform::new(c * (1.0 - scale), scale));
            mesh.rotate(Rot2::from_angle(angle), rect.center());
            list.add(clip, Shape::mesh(mesh));
        }
    });
}
