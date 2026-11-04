//! Log de latência no stderr, só com `--debug`. Nunca registra conteúdo de imagem.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn set_debug(on: bool) {
    DEBUG.store(on, Ordering::Relaxed);
}

pub fn latency(label: &str, t0: Instant) {
    if DEBUG.load(Ordering::Relaxed) {
        eprintln!("[latency] {label}: {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
    }
}
