use std::fs;
use std::thread::Builder;

use super::{ensure_file_contents, format_code, PREAMBLE};
use crate::project_root;

mod match_token;

pub fn generate(check: bool) -> anyhow::Result<()> {
    let input = project_root().join("html5ever/src/tree_builder/rules.rs");
    let output = project_root().join("html5ever/src/tree_builder/generated.rs");

    #[cfg(target_os = "haiku")]
    let stack_size = 16;

    #[cfg(not(target_os = "haiku"))]
    let stack_size = 128;

    // We have stack overflows on Servo's CI.
    let handle = Builder::new().stack_size(stack_size * 1024 * 1024).spawn(
        move || -> anyhow::Result<()> {
            let source = fs::read_to_string(input).unwrap();
            let generated = format!("{PREAMBLE}\n{}", match_token::expand(&source));
            let generated = format_code(&generated)?;

            ensure_file_contents(&output, &generated, check)?;

            Ok(())
        },
    )?;

    handle.join().unwrap()?;

    Ok(())
}
