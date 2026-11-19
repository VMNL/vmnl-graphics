// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Shared test-only synchronization for GPU tests in the library test binary.

use std::sync::{Mutex, MutexGuard, OnceLock};

static GPU_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn gpu_test_guard() -> MutexGuard<'static, ()> {
    GPU_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
