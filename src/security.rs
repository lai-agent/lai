use regex::Regex;
use serde::Deserialize;
use std::io::{self, Write};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum SecurityMode {
    Off,
    Confirm,
    Strict,
}

impl Default for SecurityMode {
    fn default() -> Self {
        Self::Confirm
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    #[serde(default)]
    pub mode: SecurityMode,

    #[serde(default = "default_blocked_commands")]
    pub blocked_commands: Vec<String>,

    #[serde(default = "default_blocked_paths")]
    pub blocked_paths: Vec<String>,

    #[serde(default = "default_true")]
    pub require_confirm_rm: bool,

    #[serde(default = "default_true")]
    pub require_confirm_sudo: bool,

    #[serde(default = "default_true")]
    pub require_confirm_write_system: bool,

    #[serde(default)]
    pub allow_network: bool,
}

fn default_blocked_commands() -> Vec<String> {
    vec![
        "rm -rf /".to_string(),
        "mkfs".to_string(),
        ":(){ :|:& };:".to_string(),
    ]
}

fn default_blocked_paths() -> Vec<String> {
    vec![
        "/etc".to_string(),
        "/boot".to_string(),
        "/sys".to_string(),
        "/proc".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::Confirm,
            blocked_commands: default_blocked_commands(),
            blocked_paths: default_blocked_paths(),
            require_confirm_rm: true,
            require_confirm_sudo: true,
            require_confirm_write_system: true,
            allow_network: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    config: SecurityConfig,
    blocked_cmd_re: Vec<Regex>,
    blocked_path_re: Vec<Regex>,
}

impl SecurityPolicy {
    pub fn new(config: SecurityConfig) -> Self {
        let blocked_cmd_re: Vec<Regex> = config
            .blocked_commands
            .iter()
            .filter_map(|c| Regex::new(&format!("(?i){}", regex::escape(c))).ok())
            .collect();

        let blocked_path_re: Vec<Regex> = config
            .blocked_paths
            .iter()
            .filter_map(|p| {
                let pattern = format!("^{}", regex::escape(p));
                Regex::new(&pattern).ok()
            })
            .collect();

        Self {
            config,
            blocked_cmd_re,
            blocked_path_re,
        }
    }

    pub fn check_code(&self, code: &str) -> Result<(), String> {
        if self.config.mode == SecurityMode::Off {
            return Ok(());
        }

        for re in &self.blocked_cmd_re {
            if re.is_match(code) {
                return Err(format!(
                    "security: blocked command pattern '{}'",
                    re.as_str()
                ));
            }
        }

        if self.config.allow_network {
            // network allowed
        } else if re_contains(code, "http") {
            return Err("security: network access disabled".to_string());
        }

        if self.config.mode == SecurityMode::Strict {
            if self.config.require_confirm_sudo && re_contains(code, "sudo") {
                return Err("security: sudo blocked in strict mode".to_string());
            }
        }

        Ok(())
    }

    pub fn confirm_dangerous(&self, code: &str) -> Result<(), String> {
        if self.config.mode == SecurityMode::Off {
            return Ok(());
        }

        let mut reasons = Vec::new();

        if self.config.require_confirm_rm && re_contains(code, "rm ") {
            reasons.push("file deletion (rm)");
        }

        if self.config.require_confirm_sudo && re_contains(code, "sudo") {
            reasons.push("sudo");
        }

        if self.config.require_confirm_write_system {
            for re in &self.blocked_path_re {
                if re.is_match(code) {
                    reasons.push("writing to system path");
                    break;
                }
            }
        }

        if reasons.is_empty() {
            return Ok(());
        }

        let reason = reasons.join(", ");
        eprint!("\n⚠ security: {} detected in: {}\n", reason, code.trim());
        eprint!("  allow? [y/N] ");
        io::stderr().flush().ok();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("read: {}", e))?;

        let input = input.trim().to_lowercase();
        if input == "y" || input == "yes" {
            Ok(())
        } else {
            Err(format!("security: blocked by user ({})", reason))
        }
    }
}

fn re_contains(text: &str, pattern: &str) -> bool {
    text.to_lowercase().contains(&pattern.to_lowercase())
}
