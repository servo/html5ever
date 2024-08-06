use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{flags, project_root};

mod html5ever;
mod markup5ever;

const PREAMBLE: &str =
    "//! This code is @generated. See `xtask/src/codegen.rs` for more information.\n";

impl flags::Codegen {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        match self.codegen_type.unwrap_or_default() {
            flags::CodegenType::All => {
                markup5ever::generate(self.check)?;
                html5ever::generate(self.check)?;
            },
            flags::CodegenType::Markup5ever => markup5ever::generate(self.check)?,
            flags::CodegenType::Html5ever => html5ever::generate(self.check)?,
        }
        Ok(())
    }
}

fn ensure_file_contents(file: &Path, contents: &str, check: bool) -> anyhow::Result<()> {
    if let Ok(old_contents) = fs::read_to_string(file) {
        if normalize_newlines(&old_contents) == normalize_newlines(contents) {
            // File is already up to date.
            return Ok(());
        }
    }

    let display_path = file.strip_prefix(project_root()).unwrap_or(file);

    if check {
        panic!(
            "{} was not up to date{}",
            display_path.display(),
            if std::env::var("CI").is_ok() {
                "\n    NOTE: run `cargo xtask codegen` locally and commit updated files.\n"
            } else {
                ""
            }
        );
    }

    eprintln!(
        "\n\x1b[31;1merror\x1b[0m: {} was not up-to-date, updating\n",
        display_path.display()
    );

    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(file, contents)?;

    Ok(())
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn format_code(code: &str) -> anyhow::Result<String> {
    let mut rustfmt = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    rustfmt.stdin.take().unwrap().write_all(code.as_bytes())?;

    let output = rustfmt.wait_with_output()?;
    assert!(output.status.success());
    Ok(String::from_utf8(output.stdout)?)
}
