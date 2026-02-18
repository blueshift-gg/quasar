#![no_std]
extern crate self as quasar;
#[macro_use]
pub mod macros;
#[macro_use]
pub mod sysvars;
pub mod cpi;
pub mod token;
pub mod traits;
pub mod checks;
pub mod accounts;
pub mod programs;
pub mod context;
pub mod error;
pub mod prelude;
pub mod entrypoint;

// Example program ID for the example module
prelude::declare_id!("22222222222222222222222222222222222222222222");
pub mod example;