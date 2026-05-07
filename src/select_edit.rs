use crate::core::{Annotation, AnnotationModel};
use crate::platform::{Color, PointF, RectF};
use crate::raster::Image;

#[derive(Clone, Debug)]
pub struct SelectEdit {
    annotations: AnnotationModel,
    color: Color,
    thickness: f32,
}

impl Default for SelectEdit {
    fn default() -> Self {
        Self {
            annotations: AnnotationModel::default(),
            color: Color::rgba(255, 92, 92, 255),
            thickness: 2.0,
        }
    }
}

impl SelectEdit {
    pub fn set_style(&mut self, color: Color, thickness: f32) {
        self.color = color;
        self.thickness = thickness.max(1.0);
    }

    pub fn add(&mut self, annotation: Annotation) {
        self.annotations.add_annotation(annotation);
    }

    pub fn draw_into(&self, image: &mut Image, origin: PointF) {
        for annotation in self.annotations.annotations() {
            image.draw_annotation(annotation, origin.x, origin.y);
        }
    }

    pub fn default_rect(&self, bounds: RectF) -> Annotation {
        Annotation::Rect(crate::core::RectAnnotation {
            bounds,
            color: self.color,
            thickness: self.thickness,
            filled: false,
        })
    }
}
