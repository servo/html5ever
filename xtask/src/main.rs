use std::env;
use std::path::PathBuf;

mod flags;

mod codegen;

fn main() -> anyhow::Result<()> {
    let flags = flags::Xtask::from_env_or_exit();

    match flags.subcommand {
        flags::XtaskCmd::Codegen(cmd) => cmd.run(),
    }
}

fn project_root() -> PathBuf {
    let dir =
        env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(dir).parent().unwrap().to_path_buf()
}
