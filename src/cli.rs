use clap::Parser;

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(
    name = "tx",
    about = "Zero-config monorepo script runner with split TUI",
    version
)]
pub struct Cli {
    /// The script name to run across monorepo workspaces (defaults to "dev")
    #[arg(default_value = "dev")]
    pub script: String,

    /// Optional directory to run in (defaults to current working directory)
    #[arg(short, long)]
    pub dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_script_is_dev() {
        let cli = Cli::parse_from(["tx"]);
        assert_eq!(cli.script, "dev");
        assert_eq!(cli.dir, None);
    }

    #[test]
    fn test_explicit_dev_script() {
        let cli = Cli::parse_from(["tx", "dev"]);
        assert_eq!(cli.script, "dev");
        assert_eq!(cli.dir, None);
    }

    #[test]
    fn test_arbitrary_script_name() {
        let cli = Cli::parse_from(["tx", "test"]);
        assert_eq!(cli.script, "test");
        assert_eq!(cli.dir, None);

        let cli2 = Cli::parse_from(["tx", "typecheck"]);
        assert_eq!(cli2.script, "typecheck");
        assert_eq!(cli2.dir, None);
    }

    #[test]
    fn test_script_with_dir_flag() {
        let cli = Cli::parse_from(["tx", "build", "--dir", "apps/backend"]);
        assert_eq!(cli.script, "build");
        assert_eq!(cli.dir, Some("apps/backend".to_string()));

        let cli2 = Cli::parse_from(["tx", "--dir", "apps/backend"]);
        assert_eq!(cli2.script, "dev");
        assert_eq!(cli2.dir, Some("apps/backend".to_string()));
    }
}
