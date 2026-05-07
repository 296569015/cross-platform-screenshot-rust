use super::Annotation;

#[derive(Clone, Debug, Default)]
pub struct AnnotationModel {
    annotations: Vec<Annotation>,
}

impl AnnotationModel {
    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    pub fn remove_annotation(&mut self, index: usize) {
        if index < self.annotations.len() {
            self.annotations.remove(index);
        }
    }

    pub fn remove_last(&mut self) -> Option<Annotation> {
        self.annotations.pop()
    }

    pub fn clear(&mut self) {
        self.annotations.clear();
    }

    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    pub fn count(&self) -> usize {
        self.annotations.len()
    }
}
