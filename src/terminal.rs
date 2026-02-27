use termwiz::escape::osc::OperatingSystemCommand;
pub use termwiz::escape::osc::Progress;

pub struct TitleGuard;

impl TitleGuard {
    pub fn new(title: &str) -> Self {
        print!("{}", OperatingSystemCommand::SetWindowTitle(title.to_string()));
        Self
    }
}

impl Drop for TitleGuard {
    fn drop(&mut self) {
        print!("{}", OperatingSystemCommand::SetWindowTitle(String::new()));
    }
}

pub fn send_notification(message: &str) {
    print!("{}", OperatingSystemCommand::SystemNotification(message.to_string()));
}

pub fn set_progress(progress: Progress) {
    print!("{}", OperatingSystemCommand::ConEmuProgress(progress));
}
