//! Solver-independent foundations for circle packing.
//!
//! Geometry and feasibility checks, contest input/output, numerical configuration,
//! and their errors stay independent of solver algorithms and browser rendering.

mod config;
mod error;
mod geometry;
mod io;
pub mod numerics;
pub mod solver;

pub use config::SolverConfig;
pub use error::PackingError;
pub use geometry::{validate, Instance, Point, Solution, ValidationReport};
pub use io::{format_solution, parse_instance, parse_solution};
