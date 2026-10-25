use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ApplicationKind {
    Web,
    Api,
    #[value(alias = "microservice")]
    Service,
    Monolith,
}

impl ApplicationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::Service => "service",
            Self::Monolith => "monolith",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Web => "a web application with a plain-text greeting route",
            Self::Api => "a JSON API with a structured greeting response",
            Self::Service => "a service with a local health sample route",
            Self::Monolith => "a modular monolith with a bounded catalog module",
        }
    }

    pub(crate) fn route(self) -> &'static str {
        match self {
            Self::Web => "/hello/Alice",
            Self::Api => "/api/greetings/Alice",
            Self::Service => "/health",
            Self::Monolith => "/catalog/items",
        }
    }

    pub(crate) fn needs_serde(self) -> bool {
        matches!(self, Self::Api | Self::Monolith)
    }

    pub(crate) fn sample_response(self) -> &'static str {
        match self {
            Self::Web => "Hello, Alice!",
            Self::Api => r#"{"message":"Hello, Alice!"}"#,
            Self::Service => "ok",
            Self::Monolith => r#"[{"id":1,"name":"Notebook"},{"id":2,"name":"Pen"}]"#,
        }
    }

    pub(crate) fn response_format(self) -> &'static str {
        if self.needs_serde() {
            "json"
        } else {
            "text"
        }
    }
}

/// Optional capabilities added to the always-enabled HTTP starter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Capability {
    Web,
    Security,
    Jwt,
    Jwks,
    Actuator,
    Scheduler,
    Bootui,
    Standard,
}

impl Capability {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Security => "security",
            Self::Jwt => "jwt",
            Self::Jwks => "jwks",
            Self::Actuator => "actuator",
            Self::Scheduler => "scheduler",
            Self::Bootui => "bootui",
            Self::Standard => "standard",
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "scafra",
    version,
    about = "Developer tooling for Scafra applications"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: CommandKind,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CommandKind {
    /// Create a compilable Scafra application.
    New {
        name: PathBuf,
        /// Select the application shape (web, api, service, or monolith).
        #[arg(long, value_enum, default_value = "web")]
        kind: ApplicationKind,
        /// Add optional framework capabilities; HTTP is always included.
        #[arg(long, value_enum, value_delimiter = ',')]
        features: Vec<Capability>,
    },
    /// Run the current application through Cargo.
    Dev {
        /// Select an exact configuration directory for the application.
        #[arg(long, value_name = "DIR", conflicts_with = "config_file")]
        config_dir: Option<PathBuf>,
        /// Select one required YAML, YML, or properties configuration file.
        #[arg(long, value_name = "FILE", conflicts_with = "config_dir")]
        config_file: Option<PathBuf>,
        /// Watch interval in milliseconds (100..=60000); defaults to 500.
        /// Overrides SCAFRA_DEV_POLL_INTERVAL_MS.
        #[arg(long, value_name = "MILLISECONDS", allow_hyphen_values = true)]
        poll_interval_ms: Option<String>,
    },
    /// Check the current application through Cargo.
    Check,
}

pub(crate) fn parse() -> Cli {
    Cli::parse()
}
