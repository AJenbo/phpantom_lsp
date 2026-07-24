//! External-tool proxy pipelines: PHPStan, PHPCS, and Mago (lint + analyze).
//!
//! Each submodule owns one pipeline's schedule function and long-lived
//! background worker task. Every worker follows the same shape: wait for
//! a notification, drain extra permits, snapshot the pending URI and file
//! content, resolve the tool's binary, run it on the blocking pool, cache
//! the results, and re-publish diagnostics for the file. At most one
//! process per tool runs at a time.
//!
//! Native diagnostic scheduling (`schedule_diagnostics`,
//! `schedule_external_diagnostics`, `schedule_diagnostics_for_open_files`)
//! stays in `diagnostics::mod` — it orchestrates these pipelines but is
//! not itself an external-tool pipeline.

pub(crate) mod mago;
pub(crate) mod phpcs;
pub(crate) mod phpstan;
