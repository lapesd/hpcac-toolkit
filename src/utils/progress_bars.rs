use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

static GLOBAL_MULTI: Mutex<Option<Arc<MultiProgress>>> = Mutex::new(None);

/// RAII guard: clears the global MultiProgress when dropped.
pub struct GlobalMultiGuard;

impl Drop for GlobalMultiGuard {
    fn drop(&mut self) {
        *GLOBAL_MULTI.lock().unwrap() = None;
    }
}

/// A tracing `io::Write` impl that routes output through `MultiProgress::println`
/// when a progress bar session is active, falling back to stdout otherwise.
/// Buffers bytes per-event and flushes as a single `println` call.
pub struct MultiProgressWriter {
    buf: Vec<u8>,
    multi: Option<Arc<MultiProgress>>,
}

impl Write for MultiProgressWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let s = String::from_utf8_lossy(&self.buf);
        let s = s.trim_end_matches('\n');
        if !s.is_empty() {
            match &self.multi {
                Some(m) => {
                    for line in s.lines() {
                        m.println(line).ok();
                    }
                }
                None => println!("{}", s),
            }
        }
        self.buf.clear();
        Ok(())
    }
}

impl Drop for MultiProgressWriter {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

pub struct ProgressTracker {
    pub progress_bar: ProgressBar,
}

impl ProgressTracker {
    /// Creates a new MultiProgress, registers it globally so the tracing stdout
    /// layer routes through it, and returns an RAII guard that unregisters on drop.
    pub fn create_multi() -> (Arc<MultiProgress>, GlobalMultiGuard) {
        let multi = Arc::new(MultiProgress::new());
        *GLOBAL_MULTI.lock().unwrap() = Some(multi.clone());
        (multi, GlobalMultiGuard)
    }

    /// Returns a writer that routes through the active global MultiProgress (if any).
    pub fn make_writer() -> MultiProgressWriter {
        let multi = GLOBAL_MULTI.lock().unwrap().clone();
        MultiProgressWriter {
            buf: Vec::new(),
            multi,
        }
    }

    pub fn new(total: u64, description: Option<&str>) -> Self {
        let progress_bar = ProgressBar::new(total);
        let message = description.unwrap_or("");

        let template = "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .progress_chars("##-"),
        );

        progress_bar.set_message(message.to_string());
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    pub fn add_to_multi(multi: &MultiProgress, total: u64, description: Option<&str>) -> Self {
        let progress_bar = multi.add(ProgressBar::new(total));
        let message = description.unwrap_or("");

        let template = "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .progress_chars("##-"),
        );

        progress_bar.set_message(message.to_string());
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    pub fn new_indeterminate(multi: &MultiProgress, description: &str) -> Self {
        let progress_bar = multi.add(ProgressBar::new_spinner());

        let template = "[{elapsed_precise}] {spinner} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );

        progress_bar.set_message(description.to_string());
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    pub fn finish_with_message(&self, msg: &str) {
        self.progress_bar.finish_with_message(msg.to_string());
    }

    pub fn set_position(&self, position: u64) {
        self.progress_bar.set_position(position);
    }

    pub fn inc(&self, steps: u64) {
        self.progress_bar.inc(steps);
    }

    pub fn update_message(&self, msg: &str) {
        self.progress_bar.set_message(msg.to_string());
    }
}
