use anyhow::Result;

use crate::{
    cli::CommandKind,
    filesystem::{run_cargo, run_dev},
    generator::create_project,
};

pub(crate) fn run(command: CommandKind) -> Result<()> {
    match command {
        CommandKind::New { name, kind } => create_project(&name, kind),
        CommandKind::Dev {
            config_dir,
            config_file,
        } => run_dev(config_dir, config_file),
        CommandKind::Check => run_cargo("check"),
    }
}
