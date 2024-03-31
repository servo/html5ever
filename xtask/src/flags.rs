use std::str::FromStr;

xflags::xflags! {
    src "./src/flags.rs"

    /// Run custom build command.
    cmd xtask {

        /// Generate code
        cmd codegen {
            /// values: [all]
            optional codegen_type: CodegenType
            optional --check
        }
    }
}

// generated start
// The following code is generated by `xflags` macro.
// Run `env UPDATE_XFLAGS=1 cargo build` to regenerate.
#[derive(Debug)]
pub struct Xtask {
    pub subcommand: XtaskCmd,
}

#[derive(Debug)]
pub enum XtaskCmd {
    Codegen(Codegen),
}

#[derive(Debug)]
pub struct Codegen {
    pub codegen_type: Option<CodegenType>,

    pub check: bool,
}

impl Xtask {
    #[allow(dead_code)]
    pub fn from_env_or_exit() -> Self {
        Self::from_env_or_exit_()
    }

    #[allow(dead_code)]
    pub fn from_env() -> xflags::Result<Self> {
        Self::from_env_()
    }

    #[allow(dead_code)]
    pub fn from_vec(args: Vec<std::ffi::OsString>) -> xflags::Result<Self> {
        Self::from_vec_(args)
    }
}
// generated end

#[derive(Debug, Default)]
pub enum CodegenType {
    #[default]
    All,
}

impl FromStr for CodegenType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            _ => Err("Invalid option".to_owned()),
        }
    }
}
