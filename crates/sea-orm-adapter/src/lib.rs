#![doc = include_str!("../README.md")]

mod action;
mod adapter;
pub mod entity;
mod migration;

pub use adapter::SeaOrmAdapter;
pub use migration::{down, up};
