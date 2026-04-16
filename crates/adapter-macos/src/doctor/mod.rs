mod automation;
mod filesystem;
mod runner;
mod types;

pub use runner::{DoctorOptions, run_doctor};
pub use types::{DoctorCheck, DoctorCheckStatus, DoctorReport};
