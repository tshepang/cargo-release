use std::io::{stdin, stdout, Write};

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub fn console_println(text: &str, color: Option<Color>, bold: bool) {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    stdout.reset().unwrap();
    // unwrap the result, panic if error
    stdout
        .set_color(ColorSpec::new().set_fg(color).set_bold(bold))
        .unwrap();
    writeln!(&mut stdout, "{}", text).unwrap();
    stdout.reset().unwrap();
}

pub fn confirm(prompt: &str) -> bool {
    let mut input = String::new();

    console_println(&format!("{} [y/N] ", prompt), None, true);

    stdout().flush().unwrap();
    stdin().read_line(&mut input).expect("y/n required");

    input.trim().to_lowercase() == "y"
}

pub fn log_info(text: &str) {
    console_println(text, Some(Color::Green), false);
}

pub fn log_warn(text: &str) {
    console_println(text, Some(Color::Red), true);
}
