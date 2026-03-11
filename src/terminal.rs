use std::sync::LazyLock;
use termwiz::escape::osc::OperatingSystemCommand;
pub use termwiz::escape::osc::Progress;

const ADVANCED_TERMINALS: [&str; 3] = ["ghostty", "iTerm.app", "WezTerm"];

static SUPPORTS_ADVANCED_FEATURES: LazyLock<bool> = LazyLock::new(|| {
    let reported_program = std::env::var("TERM_PROGRAM");
    if let Ok(reported_program) = reported_program {
        ADVANCED_TERMINALS
            .iter()
            .any(|terminal_id| reported_program == *terminal_id)
    } else {
        false
    }
});

fn supports_advanced_features() -> bool {
    *SUPPORTS_ADVANCED_FEATURES
}

pub struct TitleGuard {
    enabled: bool,
}

impl TitleGuard {
    pub fn new(title: &str) -> Self {
        let enabled = supports_advanced_features();
        if enabled {
            print!(
                "{}",
                OperatingSystemCommand::SetWindowTitle(title.to_string())
            );
        }
        Self { enabled }
    }
}

impl Drop for TitleGuard {
    fn drop(&mut self) {
        if self.enabled {
            print!("{}", OperatingSystemCommand::SetWindowTitle(String::new()));
        }
    }
}

pub fn send_notification(message: &str) {
    if supports_advanced_features() {
        print!(
            "{}",
            OperatingSystemCommand::SystemNotification(message.to_string())
        );
    }
}

pub fn set_progress(progress: Progress) {
    if supports_advanced_features() {
        print!("{}", OperatingSystemCommand::ConEmuProgress(progress));
    }
}
