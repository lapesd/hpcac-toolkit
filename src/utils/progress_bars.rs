use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// A reusable progress tracker that handles both creating and updating progress bars
/// with support for multi-progress displays
pub struct ProgressTracker {
    pub progress_bar: ProgressBar,
}

impl ProgressTracker {
    /// Creates a new MultiProgress for managing multiple progress bars
    pub fn create_multi() -> MultiProgress {
        MultiProgress::new()
    }

    /// Creates a new ProgressTracker with a standardized style
    ///
    /// # Arguments
    /// * `total` - The total number of items to process
    /// * `description` - Optional description of what's being processed
    pub fn new(total: u64, description: Option<&str>) -> Self {
        let progress_bar = ProgressBar::new(total);
        let message = description.unwrap_or("");

        // Create a clean, modern template similar to the example
        let template = "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .progress_chars("##-"),
        );

        // Set initial message if provided
        progress_bar.set_message(message.to_string());

        // Enable steady tick for smoother updates
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    /// Add this tracker to a multi-progress display
    pub fn add_to_multi(multi: &MultiProgress, total: u64, description: Option<&str>) -> Self {
        let progress_bar = multi.add(ProgressBar::new(total));
        let message = description.unwrap_or("");

        // Create a clean, modern template similar to the example
        let template = "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .progress_chars("##-"),
        );

        // Set initial message if provided
        progress_bar.set_message(message.to_string());

        // Enable steady tick for smoother updates
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    /// Create a new indeterminate tracker with unknown total
    /// This is useful when the total count isn't known in advance
    pub fn new_indeterminate(multi: &MultiProgress, description: &str) -> Self {
        // Using a spinner style instead of a progress bar
        let progress_bar = multi.add(ProgressBar::new_spinner());

        // Create a template for spinner style
        let template = "[{elapsed_precise}] {spinner} {msg}";

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );

        // Set initial message
        progress_bar.set_message(description.to_string());

        // Enable steady tick for spinner animation
        progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { progress_bar }
    }

    /// Finish with a completion message
    pub fn finish_with_message(&self, msg: &str) {
        self.progress_bar.finish_with_message(msg.to_string());
    }
    /// Set tracker step position
    pub fn set_position(&self, position: u64) {
        self.progress_bar.set_position(position);
    }

    /// Increments the progress counter by N steps
    pub fn inc(&self, steps: u64) {
        self.progress_bar.inc(steps);
    }

    /// Updates the message displayed alongside the progress bar
    pub fn update_message(&self, msg: &str) {
        self.progress_bar.set_message(msg.to_string());
    }
}
