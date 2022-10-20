use std::io::{stdin, stdout, Write};

use anyhow::Context as _;
use termcolor::{ColorChoice, StandardStream, WriteColor};

pub use termcolor::Color;
pub use termcolor::ColorSpec;

use crate::error::CargoResult;

pub fn confirm(prompt: &str) -> bool {
    let mut input = String::new();

    console_println(&format!("{} [y/N] ", prompt), None, true);

    stdout().flush().unwrap();
    stdin().read_line(&mut input).expect("y/n required");

    input.trim().to_lowercase() == "y"
}

fn console_println(text: &str, color: Option<Color>, bold: bool) {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    stdout.reset().unwrap();
    // unwrap the result, panic if error
    stdout
        .set_color(ColorSpec::new().set_fg(color).set_bold(bold))
        .unwrap();
    writeln!(&mut stdout, "{}", text).unwrap();
    stdout.reset().unwrap();
}

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

pub fn log(level: log::Level, message: impl std::fmt::Display) -> CargoResult<()> {
    match level {
        log::Level::Error => error(message),
        log::Level::Warn => warn(message),
        log::Level::Info => note(message),
        _ => {
            log::log!(level, "{}", message);
            Ok(())
        }
    }
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
