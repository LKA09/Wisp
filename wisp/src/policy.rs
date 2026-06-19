use crate::config::Config;

#[derive(Debug)]
pub enum ApprovalEvent {
    Push,
    Commit,
    AddDependency,
    DeleteFile,
    ModifyProtectedFile,
    ContinueAfterTestFailure,
    AgentDisagreement,
    RiskyCommand,
}

/// Check whether a file path is in the protected list.
pub fn is_protected_path(path: &str, config: &Config) -> bool {
    let path_lower = path.to_lowercase();
    config.policy.protected_paths.iter().any(|p| {
        path_lower == p.to_lowercase() || path_lower.contains(&p.to_lowercase())
    })
}

/// Check whether a command string matches a denied pattern.
pub fn is_denied_command(command: &str, config: &Config) -> bool {
    let cmd_lower = command.to_lowercase();
    config
        .policy
        .deny_commands
        .iter()
        .any(|d| cmd_lower.contains(&d.to_lowercase()))
}

/// Whether the given event requires explicit user approval before proceeding.
pub fn requires_approval(event: &ApprovalEvent, config: &Config) -> bool {
    let needs = |val: &str| val == "always" || val == "ask";
    match event {
        ApprovalEvent::Push => needs(&config.approval.push),
        ApprovalEvent::Commit => needs(&config.approval.commit),
        ApprovalEvent::AddDependency => needs(&config.approval.add_dependency),
        ApprovalEvent::DeleteFile => needs(&config.approval.delete_file),
        ApprovalEvent::ModifyProtectedFile => true, // always deny / require approval
        ApprovalEvent::ContinueAfterTestFailure => {
            needs(&config.approval.continue_after_test_failure)
        }
        ApprovalEvent::AgentDisagreement => true,
        ApprovalEvent::RiskyCommand => true,
    }
}
