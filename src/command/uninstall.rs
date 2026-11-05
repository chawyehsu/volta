use volta_core::error::{ErrorKind, ExitCode, Fallible};
use volta_core::session::{ActivityKind, Session};
use volta_core::tool;
use volta_core::version::VersionSpec;

use crate::command::Command;

#[derive(clap::Args)]
pub(crate) struct Uninstall {
    /// Tools to uninstall, like `node@12.11.1`, `yarn@1.22`, or `typescript`
    #[arg(value_name = "tool[@version]", required = true)]
    tools: Vec<String>,
}

impl Command for Uninstall {
    fn run(self, session: &mut Session) -> Fallible<ExitCode> {
        session.add_event_start(ActivityKind::Uninstall);

        for tool in tool::Spec::from_strings(&self.tools, "uninstall")? {
            // For packages, specifically report that we do not support uninstalling
            // specific versions. For runtimes and package managers, version specs are
            // supported and are used to select which cached versions to remove.
            if let tool::Spec::Package(_name, version) = &tool {
                let VersionSpec::None = version else {
                    return Err(ErrorKind::Unimplemented {
                        feature: "uninstalling specific versions of tools".into(),
                    }
                    .into());
                };
            }

            tool.uninstall(session)?;
        }

        session.add_event_end(ActivityKind::Uninstall, ExitCode::Success);
        Ok(ExitCode::Success)
    }
}
