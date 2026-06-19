use crate::config::Config;
use crate::git::{GitSnapshot, StatusEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    pub event: ApprovalEvent,
    pub path: Option<String>,
    pub message: String,
}

pub fn is_protected_path(path: &str, config: &Config) -> bool {
    let normalized = normalize_path(path);
    config.policy.protected_paths.iter().any(|candidate| {
        let candidate = normalize_path(candidate);
        normalized == candidate
            || normalized.starts_with(&(candidate.clone() + "/"))
            || normalized.contains(&candidate)
    }) || matches_sensitive_path(&normalized)
}

pub fn is_denied_command(command: &str, config: &Config) -> bool {
    let cmd_lower = command.to_lowercase();
    config
        .policy
        .deny_commands
        .iter()
        .any(|d| cmd_lower.contains(&d.to_lowercase()))
}

pub fn approval_decision(event: &ApprovalEvent, config: &Config) -> ApprovalDecision {
    let parse = |value: &str| match value {
        "allow" => ApprovalDecision::Allow,
        "deny" => ApprovalDecision::Deny,
        "always" | "ask" => ApprovalDecision::Ask,
        _ => ApprovalDecision::Ask,
    };

    match event {
        ApprovalEvent::Push => parse(&config.approval.push),
        ApprovalEvent::Commit => parse(&config.approval.commit),
        ApprovalEvent::AddDependency => parse(&config.approval.add_dependency),
        ApprovalEvent::DeleteFile => parse(&config.approval.delete_file),
        ApprovalEvent::ModifyProtectedFile => parse(&config.approval.modify_protected_file),
        ApprovalEvent::ContinueAfterTestFailure => {
            parse(&config.approval.continue_after_test_failure)
        }
        ApprovalEvent::AgentDisagreement | ApprovalEvent::RiskyCommand => ApprovalDecision::Deny,
    }
}

pub fn requires_approval(event: &ApprovalEvent, config: &Config) -> bool {
    !matches!(approval_decision(event, config), ApprovalDecision::Allow)
}

pub fn is_protected_branch(branch: &str, config: &Config) -> bool {
    config
        .policy
        .protected_branches
        .iter()
        .any(|candidate| branch.eq_ignore_ascii_case(candidate))
}

pub fn evaluate_snapshot_delta(
    before: &GitSnapshot,
    after: &GitSnapshot,
    delta_entries: &[StatusEntry],
    config: &Config,
) -> Vec<PolicyViolation> {
    let mut violations = Vec::new();

    if before.head != after.head {
        violations.push(PolicyViolation {
            event: ApprovalEvent::Commit,
            path: None,
            message: format!(
                "HEAD changed from {} to {}.",
                before.head.as_deref().unwrap_or("unknown"),
                after.head.as_deref().unwrap_or("unknown")
            ),
        });
    }

    for entry in delta_entries {
        let path = entry.path.clone();

        if is_deleted(entry) && requires_approval(&ApprovalEvent::DeleteFile, config) {
            violations.push(PolicyViolation {
                event: ApprovalEvent::DeleteFile,
                path: Some(path.clone()),
                message: format!("Deleted file detected: {path}"),
            });
        }

        if is_dependency_manifest(&path) && requires_approval(&ApprovalEvent::AddDependency, config)
        {
            violations.push(PolicyViolation {
                event: ApprovalEvent::AddDependency,
                path: Some(path.clone()),
                message: format!("Dependency or lockfile changed: {path}"),
            });
        }

        if is_protected_path(&path, config)
            && requires_approval(&ApprovalEvent::ModifyProtectedFile, config)
        {
            violations.push(PolicyViolation {
                event: ApprovalEvent::ModifyProtectedFile,
                path: Some(path.clone()),
                message: format!("Protected or credential-like path changed: {path}"),
            });
        }
    }

    violations
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn is_deleted(entry: &StatusEntry) -> bool {
    entry.index_status == 'D' || entry.worktree_status == 'D'
}

fn is_dependency_manifest(path: &str) -> bool {
    let normalized = normalize_path(path);
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);

    matches!(
        file_name,
        "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "cargo.toml"
            | "cargo.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "go.mod"
            | "go.sum"
            | "gemfile"
            | "gemfile.lock"
            | "composer.json"
            | "composer.lock"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    )
}

fn matches_sensitive_path(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);

    file_name == ".env"
        || file_name.starts_with(".env.")
        || file_name.ends_with(".pem")
        || file_name.ends_with(".key")
        || file_name == "id_rsa"
        || file_name == "id_ed25519"
        || file_name.contains("credential")
        || file_name.contains("secret")
        || path.starts_with(".git/")
        || path == ".git"
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalEvent, evaluate_snapshot_delta, is_dependency_manifest, is_protected_branch,
        is_protected_path,
    };
    use crate::config::{
        AgentConfig, ApprovalConfig, Config, InstructionsConfig, LanguageConfig, PolicyConfig,
        WorkflowConfig,
    };
    use crate::git::{GitSnapshot, StatusEntry};
    use std::collections::HashMap;

    fn test_config() -> Config {
        Config {
            language: LanguageConfig {
                ui: "en".into(),
                fallback: "en".into(),
                internal: "en".into(),
            },
            agents: HashMap::<String, AgentConfig>::new(),
            workflow: WorkflowConfig {
                implementer: "claude".into(),
                patcher: "codex".into(),
                reviewer: "claude".into(),
                shipper: "codex".into(),
                max_review_rounds: 2,
            },
            approval: ApprovalConfig {
                push: "deny".into(),
                commit: "ask".into(),
                add_dependency: "ask".into(),
                delete_file: "ask".into(),
                modify_protected_file: "deny".into(),
                continue_after_test_failure: "ask".into(),
            },
            instructions: InstructionsConfig {
                files: vec![],
                max_bytes: 0,
                include_agent_specific: false,
            },
            policy: PolicyConfig {
                protected_branches: vec!["main".into()],
                protected_paths: vec![".env".into(), ".git".into(), "credentials.json".into()],
                deny_commands: vec![],
            },
        }
    }

    fn snapshot(head: &str) -> GitSnapshot {
        GitSnapshot {
            branch: Some("feature/test".into()),
            head: Some(head.into()),
            status_raw: String::new(),
            status_entries: Vec::new(),
            diff_name_status: String::new(),
            diff_full: String::new(),
            diff_cached: String::new(),
        }
    }

    #[test]
    fn protected_path_matches_env_and_git() {
        let config = test_config();
        assert!(is_protected_path(".env.local", &config));
        assert!(is_protected_path(".git/config", &config));
        assert!(is_protected_path("config/credentials.json", &config));
        assert!(!is_protected_path("src/main.rs", &config));
    }

    #[test]
    fn dependency_manifest_detection_matches_lockfiles() {
        assert!(is_dependency_manifest("Cargo.lock"));
        assert!(is_dependency_manifest("frontend/package.json"));
        assert!(!is_dependency_manifest("src/main.rs"));
    }

    #[test]
    fn protected_branch_match_is_case_insensitive() {
        assert!(is_protected_branch("MAIN", &test_config()));
    }

    #[test]
    fn snapshot_delta_flags_commit_and_risky_files() {
        let config = test_config();
        let before = snapshot("aaa111");
        let after = snapshot("bbb222");
        let delta = vec![
            StatusEntry {
                index_status: 'M',
                worktree_status: ' ',
                path: "Cargo.lock".into(),
            },
            StatusEntry {
                index_status: 'D',
                worktree_status: ' ',
                path: ".env".into(),
            },
        ];

        let violations = evaluate_snapshot_delta(&before, &after, &delta, &config);
        assert!(violations.iter().any(|v| v.event == ApprovalEvent::Commit));
        assert!(
            violations
                .iter()
                .any(|v| v.event == ApprovalEvent::AddDependency)
        );
        assert!(
            violations
                .iter()
                .any(|v| v.event == ApprovalEvent::DeleteFile)
        );
        assert!(
            violations
                .iter()
                .any(|v| v.event == ApprovalEvent::ModifyProtectedFile)
        );
    }
}
