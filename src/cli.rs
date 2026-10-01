use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "tx",
    about = "Zero-config monorepo dev runner with split TUI",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Run all dev commands across monorepo workspaces
    Dev {
        /// Optional directory to run in (defaults to current working directory)
        #[arg(short, long)]
        dir: Option<String>,
    },
}

impl Cli {
    pub fn resolved_command(self) -> Commands {
        self.command.unwrap_or(Commands::Dev { dir: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_command_is_dev() {
        let cli = Cli::parse_from(["tx"]);
        assert_eq!(cli.resolved_command(), Commands::Dev { dir: None });
    }

    #[test]
    fn test_explicit_dev_command() {
        let cli = Cli::parse_from(["tx", "dev"]);
        assert_eq!(cli.resolved_command(), Commands::Dev { dir: None });
    }

    #[test]
    fn test_dev_with_dir_flag() {
        let cli = Cli::parse_from(["tx", "dev", "--dir", "apps/backend"]);
        assert_eq!(
            cli.resolved_command(),
            Commands::Dev {
                dir: Some("apps/backend".to_string())
            }
        );
    }
}
