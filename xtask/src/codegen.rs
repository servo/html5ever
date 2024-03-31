use std::fs;
use std::path::Path;

use crate::{flags, project_root};

impl flags::Codegen {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        Ok(())
    }
}

fn ensure_file_contents(file: &Path, contents: &str, check: bool) {
    if let Ok(old_contents) = fs::read_to_string(file) {
        if normalize_newlines(&old_contents) == normalize_newlines(contents) {
            // File is already up to date.
            return;
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
    fs::write(file, contents).unwrap();
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}
