pub mod executor;
pub mod file;
pub mod instance;
pub mod job;
pub mod manage;
pub mod migration;
pub mod role;
pub mod tag;
pub mod team;
pub mod terminal;
pub mod user;
pub mod workflow;

mod utils;

use poem_openapi::{Tags, Validator};
use std::fmt::{self, Display, Formatter};

pub fn default_page() -> u64 {
    1
}

pub fn default_page_size() -> u64 {
    20
}

pub fn default_option_page() -> Option<u64> {
    Some(1)
}

pub fn default_option_page_size() -> Option<u64> {
    Some(20)
}

#[derive(Tags)]
pub enum Tag {
    User,
    Team,
    Job,
    Executor,
    Instance,
    File,
    Role,
    Admin,
    Migration,
    Tag,
    Workflow,
}

pub struct OneOfValidator(Vec<String>);

impl OneOfValidator {
    pub fn new(v: Vec<&str>) -> Self {
        Self(v.into_iter().map(|v| v.to_owned()).collect())
    }
}

impl Display for OneOfValidator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(format!("OneOfValidator: {:?}", self.0).as_str())
    }
}

impl Validator<String> for OneOfValidator {
    fn check(&self, value: &String) -> bool {
        self.0.contains(value)
    }
}
