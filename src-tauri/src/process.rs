use std::ffi::OsStr;
use std::process::Command;

/// Creates a child process without allocating a transient console window on
/// Windows. Captured stdout/stderr still works, and GUI applications (such as
/// Minecraft itself) can create their own normal windows.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
