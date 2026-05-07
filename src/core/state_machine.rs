use crate::platform::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppState {
    #[default]
    Idle,
    Capturing,
    Selecting,
    Annotating,
    Saving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEvent {
    HotkeyTriggered,
    FrameAcquired,
    MouseDown,
    MouseMove,
    MouseUp,
    KeyPress,
    ToolSelected,
    SaveRequested,
    CopyRequested,
    CancelRequested,
    Escape,
    SaveComplete,
}

#[derive(Clone, Debug, Default)]
pub struct ScreenshotStateMachine {
    state: AppState,
    selected_region: Rect,
}

impl ScreenshotStateMachine {
    pub fn current_state(&self) -> AppState {
        self.state
    }

    pub fn transition(&mut self, event: AppEvent) -> AppState {
        match self.state {
            AppState::Idle => {
                if event == AppEvent::HotkeyTriggered {
                    self.state = AppState::Capturing;
                }
            }
            AppState::Capturing => match event {
                AppEvent::FrameAcquired => self.state = AppState::Selecting,
                AppEvent::Escape | AppEvent::CancelRequested => self.state = AppState::Idle,
                _ => {}
            },
            AppState::Selecting => match event {
                AppEvent::MouseUp => self.state = AppState::Annotating,
                AppEvent::Escape | AppEvent::CancelRequested => self.state = AppState::Idle,
                _ => {}
            },
            AppState::Annotating => match event {
                AppEvent::SaveRequested | AppEvent::CopyRequested => self.state = AppState::Saving,
                AppEvent::Escape | AppEvent::CancelRequested => self.state = AppState::Idle,
                _ => {}
            },
            AppState::Saving => {
                if event == AppEvent::SaveComplete {
                    self.state = AppState::Idle;
                }
            }
        }
        self.state
    }

    pub fn is_selecting(&self) -> bool {
        self.state == AppState::Selecting
    }

    pub fn is_annotating(&self) -> bool {
        self.state == AppState::Annotating
    }

    pub fn set_selected_region(&mut self, rect: Rect) {
        self.selected_region = rect;
    }

    pub fn selected_region(&self) -> Rect {
        self.selected_region
    }
}
