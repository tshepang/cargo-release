use std::io::Write;

use anyhow::Context as _;
pub use termcolor::{Color, ColorChoice};
use termcolor::{ColorSpec, StandardStream, WriteColor};

use crate::error::CargoResult;

/// Whether to color logged output
fn colorize_stderr() -> ColorChoice {
    if concolor_control::get(concolor_control::Stream::Stderr).color() {
        ColorChoice::Always
    } else {
        ColorChoice::Never
    }
}

/// Print a message with a colored title in the style of Cargo shell messages.
pub fn print(
    status: &str,
    message: impl std::fmt::Display,
    color: Color,
    justified: bool,
) -> CargoResult<()> {
    let color_choice = colorize_stderr();
    let mut output = StandardStream::stderr(color_choice);

    output.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true))?;
    if justified {
        write!(output, "{status:>12}")?;
    } else {
        write!(output, "{}", status)?;
        output.set_color(ColorSpec::new().set_bold(true))?;
        write!(output, ":")?;
    }
    output.reset()?;

    writeln!(output, " {message}").with_context(|| "Failed to write message")?;

    Ok(())
}

/// Print a styled action message.
pub fn status(action: &str, message: impl std::fmt::Display) -> CargoResult<()> {
    print(action, message, Color::Green, true)
}

/// Print a styled error message.
pub fn error(message: impl std::fmt::Display) -> CargoResult<()> {
    print("error", message, Color::Red, false)
}

/// Print a styled warning message.
pub fn warn(message: impl std::fmt::Display) -> CargoResult<()> {
    print("warning", message, Color::Yellow, false)
}

/// Print a styled warning message.
pub fn note(message: impl std::fmt::Display) -> CargoResult<()> {
    print("note", message, Color::Cyan, false)
}

/// Print a part of a line with formatting
pub fn write_stderr(fragment: impl std::fmt::Display, spec: &ColorSpec) -> CargoResult<()> {
    let color_choice = colorize_stderr();
    let mut output = StandardStream::stderr(color_choice);

    output.set_color(spec)?;
    write!(output, "{}", fragment)?;
    output.reset()?;
    Ok(())
}
