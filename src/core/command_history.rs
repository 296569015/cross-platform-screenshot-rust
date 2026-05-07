use super::{Annotation, AnnotationModel};

#[derive(Clone, Debug, Default)]
pub struct CommandHistory {
    undo_stack: Vec<Annotation>,
    redo_stack: Vec<Annotation>,
}

impl CommandHistory {
    pub fn execute(&mut self, annotation: Annotation, model: &mut AnnotationModel) {
        self.redo_stack.clear();
        model.add_annotation(annotation.clone());
        self.undo_stack.push(annotation);
    }

    pub fn undo(&mut self, model: &mut AnnotationModel) {
        if let Some(annotation) = self.undo_stack.pop() {
            model.remove_last();
            self.redo_stack.push(annotation);
        }
    }

    pub fn redo(&mut self, model: &mut AnnotationModel) {
        if let Some(annotation) = self.redo_stack.pop() {
            model.add_annotation(annotation.clone());
            self.undo_stack.push(annotation);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
