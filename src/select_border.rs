use crate::platform::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseStatus {
    #[default]
    Normal,
    LBtnDown,
    Move,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatchStatus {
    #[default]
    Init,
    SelStart,
    SelIng,
    SelFinish,
    DrawIng,
    Gifing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarCommand {
    Rectangle,
    Ellipse,
    Text,
    Number,
    Pen,
    Arrow,
    Line,
    DashedLine,
    Mosaic,
    Gif,
    Pin,
    Ocr,
    LongScreenshot,
    Undo,
    Save,
    Cancel,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolbarButton {
    pub command: ToolbarCommand,
    pub rect: Rect,
    pub visible: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SelectBorder {
    buttons: Vec<ToolbarButton>,
}

impl SelectBorder {
    pub fn set_buttons(&mut self, buttons: Vec<ToolbarButton>) {
        self.buttons = buttons;
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<ToolbarCommand> {
        self.buttons
            .iter()
            .find(|button| button.visible && button.rect.contains(crate::platform::Point { x, y }))
            .map(|button| button.command)
    }

    pub fn buttons(&self) -> &[ToolbarButton] {
        &self.buttons
    }
}
