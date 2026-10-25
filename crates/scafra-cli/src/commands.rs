use anyhow::Result;

use crate::{
    cli::CommandKind,
    filesystem::{run_cargo, run_dev},
    generator::create_project,
};

pub(crate) fn run(command: CommandKind) -> Result<()> {
    match command {
        CommandKind::New {
            name,
            kind,
            features,
        } => create_project(&name, kind, &features),
        CommandKind::Dev {
            config_dir,
            config_file,
            poll_interval_ms,
        } => run_dev(config_dir, config_file, poll_interval_ms),
        CommandKind::Check => run_cargo("check"),
    }
}
