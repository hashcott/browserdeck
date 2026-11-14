//! Live CPU, RAM and temperature for usage keys, pushed to the phone while one is connected
//! and some page has a usage key.
use crate::config::{Action, Config};
use crate::hub::ServerMsg;
use crate::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Components, System};
use tauri::async_runtime::block_on;

const INTERVAL: Duration = Duration::from_secs(2);

/// Whole numbers: CPU and RAM in percent, temperature in °C. None: this machine does not report it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Usage {
    pub cpu: Option<u8>,
    pub ram: Option<u8>,
    pub temp: Option<i16>,
}

struct Sampler {
    sys: System,
    components: Components,
}

impl Sampler {
    /// Takes the first CPU reading; usage is the change since the previous refresh.
    fn new() -> Sampler {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        Sampler { sys, components: Components::new_with_refreshed_list() }
    }

    fn sample(&mut self) -> Usage {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.components.refresh(false);
        let total = self.sys.total_memory();
        let pct = |v: f64| v.round().clamp(0.0, 100.0) as u8;
        Usage {
            cpu: Some(pct(self.sys.global_cpu_usage() as f64)),
            ram: (total > 0).then(|| pct(self.sys.used_memory() as f64 * 100.0 / total as f64)),
            // ponytail: hottest sensor stands in for the CPU; pick sensors by label if a GPU or VRM reads higher.
            temp: self
                .components
                .iter()
                .filter_map(|c| c.temperature())
                .filter(|t| (1.0..=150.0).contains(t))
                .reduce(f32::max)
                .map(|t| t.round() as i16),
        }
    }
}

fn wanted(c: &Config) -> bool {
    c.pages.iter().flat_map(|p| &p.buttons).any(|b| matches!(b.action, Action::Usage { .. }))
}

/// Its own thread: sysinfo reads Windows sensors over COM and keeps that COM setup on the thread it ran on.
pub fn spawn(state: Arc<AppState>) {
    let _ = std::thread::Builder::new().name("usage".into()).spawn(move || {
        let mut sampler: Option<Sampler> = None;
        loop {
            std::thread::sleep(INTERVAL);
            if !block_on(state.hub.connected()) || !wanted(&state.config.blocking_read()) {
                continue;
            }
            // A new sampler's first CPU figure would cover no time at all, so it waits a tick.
            let Some(s) = sampler.as_mut() else {
                sampler = Some(Sampler::new());
                continue;
            };
            block_on(state.hub.send(ServerMsg::Usage(s.sample())));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Metric;

    #[test]
    fn a_sample_is_in_range() {
        let mut s = Sampler::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let u = s.sample();
        assert!(u.cpu.is_some_and(|v| v <= 100));
        assert!(u.ram.is_some_and(|v| (1..=100).contains(&v)));
        assert!(u.temp.is_none_or(|t| (1..=150).contains(&t)));
    }

    #[test]
    fn only_wanted_with_a_usage_key() {
        let mut c = Config::default_with_token("t".into());
        assert!(!wanted(&c));
        c.pages[0].buttons[0].action = Action::Usage { metric: Metric::Cpu };
        assert!(wanted(&c));
    }

    #[test]
    fn the_phone_gets_a_flat_usage_message() {
        let m = ServerMsg::Usage(Usage { cpu: Some(12), ram: Some(48), temp: None });
        assert_eq!(serde_json::to_value(m).unwrap(), serde_json::json!({ "type": "usage", "cpu": 12, "ram": 48, "temp": null }));
    }
}
