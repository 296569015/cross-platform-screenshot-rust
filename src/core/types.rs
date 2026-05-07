use crate::platform::{Color, PointF, RectF};

#[derive(Clone, Debug, PartialEq)]
pub struct RectAnnotation {
    pub bounds: RectF,
    pub color: Color,
    pub thickness: f32,
    pub filled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrowAnnotation {
    pub start: PointF,
    pub end: PointF,
    pub color: Color,
    pub thickness: f32,
    pub head_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextAnnotation {
    pub position: PointF,
    pub text: String,
    pub color: Color,
    pub font_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineAnnotation {
    pub start: PointF,
    pub end: PointF,
    pub color: Color,
    pub thickness: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FreehandAnnotation {
    pub points: Vec<PointF>,
    pub color: Color,
    pub thickness: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Rect(RectAnnotation),
    Arrow(ArrowAnnotation),
    Text(TextAnnotation),
    Line(LineAnnotation),
    Freehand(FreehandAnnotation),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnotationTool {
    None,
    #[default]
    Rectangle,
    Arrow,
    Text,
    Line,
    Freehand,
}
