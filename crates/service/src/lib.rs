pub mod logic;
pub mod state;
use chrono::Local;
pub use entity;
use nanoid::nanoid;
pub mod config;

pub struct IdGenerator;

impl IdGenerator {
    const JOB_PREFIX: &'static str = "j";
    const JOB_BUNDLE_SCRIPT_PREFIX: &'static str = "b";
    const TIMER_JOB_PREFIX: &'static str = "t";
    const FLOW_JOB_PREFIX: &'static str = "f";
    const SCHEDULE_ID_PREFIX: &'static str = "s";
    const INSTANCE_PREFIX: &'static str = "i";

    pub fn get_job_eid() -> String {
        Self::get_id(Self::JOB_PREFIX)
    }

    pub fn get_job_bundle_script_uid() -> String {
        Self::get_id(Self::JOB_BUNDLE_SCRIPT_PREFIX)
    }

    pub fn get_timer_uid() -> String {
        Self::get_id(Self::TIMER_JOB_PREFIX)
    }

    pub fn get_flow_job_uid() -> String {
        Self::get_id(Self::FLOW_JOB_PREFIX)
    }
    pub fn get_schedule_uid() -> String {
        Self::get_id(Self::SCHEDULE_ID_PREFIX)
    }

    pub fn get_instance_uid() -> String {
        Self::get_id(Self::INSTANCE_PREFIX)
    }

    fn get_id(prefix: &str) -> String {
        format!("{prefix}-{}", nanoid!(10)).into()
    }

    pub fn get_run_id() -> String {
        Local::now().format("%Y%m%d%H%M%S").to_string()
    }
}
