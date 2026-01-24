use std::collections::VecDeque;

use crate::data::DeviceStatus;

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    pub status: Option<DeviceStatus>,
    pub connected: bool,
    pub flow_history: VecDeque<(f64, f64)>,
    pub speed_history: VecDeque<(f64, f64)>,
    pub target_flow: u16,
    pub tick: u32,
    pub should_quit: bool,
    pub simulate: bool,
    pub read_only: bool,
    pub show_debug: bool,
    pub input_mode: bool,
    pub input_buffer: String,
}

impl AppState {
    pub fn new(simulate: bool, read_only: bool) -> Self {
        Self {
            status: None,
            connected: false,
            flow_history: VecDeque::with_capacity(120),
            speed_history: VecDeque::with_capacity(120),
            target_flow: 0,
            tick: 0,
            should_quit: false,
            simulate,
            read_only,
            show_debug: false,
            input_mode: false,
            input_buffer: String::new(),
        }
    }

    pub fn update_status(&mut self, status: DeviceStatus) {
        self.target_flow = status.target_flow;
        self.status = Some(status);
        self.connected = true;
        self.push_history();
    }

    fn push_history(&mut self) {
        let tick = f64::from(self.tick);
        if let Some(status) = &self.status {
            self.flow_history
                .push_back((tick, f64::from(status.real_flow)));
            self.flow_history
                .push_back((tick, f64::from(status.target_flow)));
            self.speed_history
                .push_back((tick, f64::from(status.speed_rpm)));
        }
        self.tick = self.tick.wrapping_add(1);
        while self.flow_history.len() > 240 {
            self.flow_history.pop_front();
        }
        while self.speed_history.len() > 120 {
            self.speed_history.pop_front();
        }
    }
}
