use std::time::{Duration, Instant};

use super::session::{
    LongShotAppendOutcome, LongShotAppendStatus, LongShotSession, LongShotSessionOptions,
    LongShotStartError,
};
use super::types::{LongShotImage, LongShotPoint, LongShotRect};
use super::windows::{
    capture_region_or_covered_window, post_mouse_wheel_at, LongShotCaptureRequest,
    LongShotCaptureResponse, LongShotCaptureWorker,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LongShotRuntimeConfig {
    pub capture_delay: Duration,
    pub min_capture_interval: Duration,
    pub auto_scroll_interval: Duration,
    pub auto_capture_delay: Duration,
    pub auto_capture_interval: Duration,
    pub auto_wheel_delta: i32,
    pub max_auto_stalls: i32,
}

impl Default for LongShotRuntimeConfig {
    fn default() -> Self {
        Self {
            capture_delay: Duration::from_millis(50),
            min_capture_interval: Duration::from_millis(35),
            auto_scroll_interval: Duration::from_millis(120),
            auto_capture_delay: Duration::from_millis(45),
            auto_capture_interval: Duration::from_millis(55),
            auto_wheel_delta: -40,
            max_auto_stalls: 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LongShotRuntimeEvent {
    Started {
        image: LongShotImage,
    },
    PreviewUpdated {
        image: LongShotImage,
        outcome: LongShotAppendOutcome,
    },
    FrameIgnored {
        outcome: LongShotAppendOutcome,
    },
    CaptureFailed {
        seq: u64,
    },
    AutoScrollStopped,
    MaxOutputHeightReached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongShotRuntimeError {
    InvalidRegion,
    InitialCaptureFailed,
    Start(LongShotStartError),
}

pub struct LongShotRuntime {
    session: LongShotSession,
    worker: Option<LongShotCaptureWorker>,
    config: LongShotRuntimeConfig,
    desktop_bounds: LongShotRect,
    overlay_hwnd: isize,
    source_region: LongShotRect,
    generation: u64,
    pending_capture: bool,
    pending_seq: u64,
    capture_due: Instant,
    last_frame_capture: Instant,
    capture_in_flight: bool,
    capture_in_flight_seq: u64,
    auto_scroll_active: bool,
    auto_stalls: i32,
    next_auto_scroll: Instant,
    last_scroll_target: isize,
}

impl Default for LongShotRuntime {
    fn default() -> Self {
        Self::new(LongShotRuntimeConfig::default())
    }
}

impl LongShotRuntime {
    pub fn new(config: LongShotRuntimeConfig) -> Self {
        let now = Instant::now();
        Self {
            session: LongShotSession::default(),
            worker: LongShotCaptureWorker::spawn(),
            config,
            desktop_bounds: LongShotRect::default(),
            overlay_hwnd: 0,
            source_region: LongShotRect::default(),
            generation: 0,
            pending_capture: false,
            pending_seq: 0,
            capture_due: now,
            last_frame_capture: now,
            capture_in_flight: false,
            capture_in_flight_seq: 0,
            auto_scroll_active: false,
            auto_stalls: 0,
            next_auto_scroll: now,
            last_scroll_target: 0,
        }
    }

    pub fn start(
        &mut self,
        desktop_bounds: LongShotRect,
        overlay_hwnd: isize,
        source_region: LongShotRect,
    ) -> Result<LongShotRuntimeEvent, LongShotRuntimeError> {
        let options = LongShotSessionOptions::tuned_for_region_height(source_region.h);
        self.start_with_options(desktop_bounds, overlay_hwnd, source_region, options)
    }

    pub fn start_with_options(
        &mut self,
        desktop_bounds: LongShotRect,
        overlay_hwnd: isize,
        source_region: LongShotRect,
        options: LongShotSessionOptions,
    ) -> Result<LongShotRuntimeEvent, LongShotRuntimeError> {
        if desktop_bounds.is_empty() || source_region.is_empty() {
            self.reset();
            return Err(LongShotRuntimeError::InvalidRegion);
        }
        let Some(frame) =
            capture_region_or_covered_window(desktop_bounds, overlay_hwnd, source_region)
        else {
            self.reset();
            return Err(LongShotRuntimeError::InitialCaptureFailed);
        };

        self.session
            .start(source_region, frame, options)
            .map_err(LongShotRuntimeError::Start)?;

        let now = Instant::now();
        self.desktop_bounds = desktop_bounds;
        self.overlay_hwnd = overlay_hwnd;
        self.source_region = source_region;
        self.generation = self.generation.saturating_add(1);
        self.pending_capture = false;
        self.pending_seq = 0;
        self.capture_due = now;
        self.last_frame_capture = now;
        self.capture_in_flight = false;
        self.capture_in_flight_seq = 0;
        self.auto_scroll_active = false;
        self.auto_stalls = 0;
        self.next_auto_scroll = now;
        self.last_scroll_target = 0;

        let image = self
            .session
            .output_image()
            .ok_or(LongShotRuntimeError::InitialCaptureFailed)?;
        Ok(LongShotRuntimeEvent::Started { image })
    }

    pub fn reset(&mut self) {
        let now = Instant::now();
        self.session.reset();
        self.pending_capture = false;
        self.pending_seq = 0;
        self.capture_due = now;
        self.last_frame_capture = now;
        self.capture_in_flight = false;
        self.capture_in_flight_seq = 0;
        self.auto_scroll_active = false;
        self.auto_stalls = 0;
        self.next_auto_scroll = now;
        self.last_scroll_target = 0;
        self.generation = self.generation.saturating_add(1);
    }

    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    pub fn is_auto_scroll_active(&self) -> bool {
        self.auto_scroll_active
    }

    pub fn source_region(&self) -> LongShotRect {
        self.source_region
    }

    pub fn output_image(&self) -> Option<LongShotImage> {
        self.session.output_image()
    }

    pub fn handle_manual_wheel(&mut self, screen_point: LongShotPoint, wheel_delta: i32) -> bool {
        if !self.is_active() || wheel_delta >= 0 {
            return false;
        }

        if !post_mouse_wheel_at(
            screen_point,
            wheel_delta,
            self.overlay_hwnd,
            &mut self.last_scroll_target,
        ) {
            return false;
        }

        let seq = self.session.next_scroll_seq();
        self.schedule_capture(
            seq,
            Instant::now(),
            self.config.capture_delay,
            self.config.min_capture_interval,
        );
        true
    }

    pub fn set_auto_scroll(&mut self, active: bool) {
        if !self.is_active() {
            self.auto_scroll_active = false;
            return;
        }
        self.auto_scroll_active = active;
        self.auto_stalls = 0;
        self.next_auto_scroll = Instant::now();
    }

    pub fn toggle_auto_scroll(&mut self) -> bool {
        let active = !self.auto_scroll_active;
        self.set_auto_scroll(active);
        self.auto_scroll_active
    }

    pub fn tick(&mut self) -> Vec<LongShotRuntimeEvent> {
        let mut events = self.poll_capture_worker();
        let now = Instant::now();

        if self.auto_scroll_active && now >= self.next_auto_scroll {
            events.extend(self.run_auto_scroll_step(now));
        }

        if self.pending_capture && !self.capture_in_flight && now >= self.capture_due {
            events.extend(self.dispatch_pending_capture(now));
        }

        events.extend(self.poll_capture_worker());
        events
    }

    pub fn finish_for_export(&mut self, wait_timeout: Duration) -> Option<LongShotImage> {
        if !self.is_active() {
            return None;
        }

        self.auto_scroll_active = false;
        let _ = self.tick();
        self.wait_for_in_flight_capture(wait_timeout);

        if !self.capture_in_flight && self.session.can_accept_frame_height(self.source_region.h) {
            let seq = self.session.next_scroll_seq();
            if let Some(frame) = capture_region_or_covered_window(
                self.desktop_bounds,
                self.overlay_hwnd,
                self.source_region,
            ) {
                let _ = self.session.append_frame_for_scroll(seq, frame);
            }
        }

        self.session.output_image()
    }

    fn run_auto_scroll_step(&mut self, now: Instant) -> Vec<LongShotRuntimeEvent> {
        self.next_auto_scroll = now + self.config.auto_scroll_interval;
        let point = self.auto_scroll_point();
        if post_mouse_wheel_at(
            point,
            self.config.auto_wheel_delta,
            self.overlay_hwnd,
            &mut self.last_scroll_target,
        ) {
            let seq = self.session.next_scroll_seq();
            self.schedule_capture(
                seq,
                now,
                self.config.auto_capture_delay,
                self.config.auto_capture_interval,
            );
            Vec::new()
        } else {
            self.record_auto_stall()
        }
    }

    fn schedule_capture(
        &mut self,
        seq: u64,
        scroll_at: Instant,
        capture_delay: Duration,
        min_capture_interval: Duration,
    ) {
        self.pending_seq = seq;
        let requested_due = scroll_at + capture_delay;
        let interval_due = self.last_frame_capture + min_capture_interval;
        let next_due = requested_due.max(interval_due);
        self.capture_due = if self.pending_capture {
            self.capture_due.min(next_due)
        } else {
            next_due
        };
        self.pending_capture = true;
    }

    fn dispatch_pending_capture(&mut self, now: Instant) -> Vec<LongShotRuntimeEvent> {
        self.pending_capture = false;
        self.last_frame_capture = now;

        if !self.session.can_accept_frame_height(self.source_region.h) {
            self.auto_scroll_active = false;
            return vec![LongShotRuntimeEvent::MaxOutputHeightReached];
        }

        let request = LongShotCaptureRequest {
            generation: self.generation,
            seq: self.pending_seq,
            desktop_bounds: self.desktop_bounds,
            overlay_hwnd: self.overlay_hwnd,
            region: self.source_region,
        };

        if let Some(worker) = &self.worker {
            if worker.request(request) {
                self.capture_in_flight = true;
                self.capture_in_flight_seq = request.seq;
                return Vec::new();
            }
        }

        let frame = capture_region_or_covered_window(
            request.desktop_bounds,
            request.overlay_hwnd,
            request.region,
        );
        self.handle_capture_response(LongShotCaptureResponse {
            generation: request.generation,
            seq: request.seq,
            frame,
        })
    }

    fn poll_capture_worker(&mut self) -> Vec<LongShotRuntimeEvent> {
        let responses = self
            .worker
            .as_ref()
            .map(LongShotCaptureWorker::drain_responses)
            .unwrap_or_default();

        let mut events = Vec::new();
        for response in responses {
            events.extend(self.handle_capture_response(response));
        }
        events
    }

    fn wait_for_in_flight_capture(&mut self, timeout: Duration) {
        if !self.capture_in_flight {
            return;
        }

        let Some(response) = self
            .worker
            .as_ref()
            .and_then(|worker| worker.wait_for_response(timeout))
        else {
            return;
        };
        let _ = self.handle_capture_response(response);
    }

    fn handle_capture_response(
        &mut self,
        response: LongShotCaptureResponse,
    ) -> Vec<LongShotRuntimeEvent> {
        if response.generation != self.generation {
            return Vec::new();
        }
        if self.capture_in_flight && response.seq != self.capture_in_flight_seq {
            return Vec::new();
        }

        self.capture_in_flight = false;
        let Some(frame) = response.frame else {
            return self.capture_failed(response.seq);
        };

        let outcome = self.session.append_frame_for_scroll(response.seq, frame);
        match outcome.status {
            LongShotAppendStatus::Appended => {
                self.auto_stalls = 0;
                if let Some(image) = self.session.output_image() {
                    vec![LongShotRuntimeEvent::PreviewUpdated { image, outcome }]
                } else {
                    Vec::new()
                }
            }
            LongShotAppendStatus::TooTall => {
                self.auto_scroll_active = false;
                vec![LongShotRuntimeEvent::MaxOutputHeightReached]
            }
            LongShotAppendStatus::Duplicate | LongShotAppendStatus::Rejected => {
                let mut events = vec![LongShotRuntimeEvent::FrameIgnored { outcome }];
                events.extend(self.record_auto_stall());
                events
            }
            _ => vec![LongShotRuntimeEvent::FrameIgnored { outcome }],
        }
    }

    fn capture_failed(&mut self, seq: u64) -> Vec<LongShotRuntimeEvent> {
        let mut events = vec![LongShotRuntimeEvent::CaptureFailed { seq }];
        events.extend(self.record_auto_stall());
        events
    }

    fn record_auto_stall(&mut self) -> Vec<LongShotRuntimeEvent> {
        if !self.auto_scroll_active {
            return Vec::new();
        }
        self.auto_stalls += 1;
        if self.auto_stalls >= self.config.max_auto_stalls {
            self.auto_scroll_active = false;
            vec![LongShotRuntimeEvent::AutoScrollStopped]
        } else {
            Vec::new()
        }
    }

    fn auto_scroll_point(&self) -> LongShotPoint {
        LongShotPoint {
            x: self.desktop_bounds.x + self.source_region.x + self.source_region.w / 2,
            y: self.desktop_bounds.y + self.source_region.y + self.source_region.h / 2,
        }
    }
}
