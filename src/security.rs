use regex::Regex;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
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

impl From<&str> for SecurityMode {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => Self::Off,
            "strict" => Self::Strict,
            _ => Self::Confirm,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub mode: SecurityMode,
    pub blocked_commands: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub blocked_functions: Vec<String>,
    pub require_confirm_rm: bool,
    pub require_confirm_sudo: bool,
    pub require_confirm_write_system: bool,
    pub require_confirm_eval: bool,
    pub allow_network: bool,
    pub blocked_domains: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub sandbox_paths: Vec<String>,
    pub max_ops_per_turn: usize,
    pub max_output_bytes: usize,
    pub exec_timeout_secs: u64,
    pub audit_log: Option<String>,
}



impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::Confirm,
            blocked_commands: vec![
                "rm -rf /".to_string(),
                "mkfs".to_string(),
                ":(){ :|:& };:".to_string(),
            ],
            blocked_paths: vec![
                "/etc".to_string(),
                "/boot".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
            ],
            blocked_functions: vec![
                "exit".to_string(),
                "setenv".to_string(),
            ],
            require_confirm_rm: true,
            require_confirm_sudo: true,
            require_confirm_write_system: true,
            require_confirm_eval: true,
            allow_network: true,
            blocked_domains: vec![],
            allowed_domains: vec![],
            sandbox_paths: vec![],
            max_ops_per_turn: 50,
            max_output_bytes: 1024 * 1024,
            exec_timeout_secs: 60,
            audit_log: None,
        }
    }
}

#[derive(Debug)]
pub struct SecurityPolicy {
    config: SecurityConfig,
    blocked_cmd_re: Vec<Regex>,
    blocked_path_re: Vec<Regex>,
    blocked_func_re: Vec<Regex>,
    blocked_domain_re: Vec<Regex>,
    allowed_domain_re: Vec<Regex>,
    sandbox_re: Vec<Regex>,
    op_count: AtomicUsize,
    turn_start: Mutex<Option<Instant>>,
    audit_path: Option<PathBuf>,
}

impl Clone for SecurityPolicy {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            blocked_cmd_re: self.blocked_cmd_re.clone(),
            blocked_path_re: self.blocked_path_re.clone(),
            blocked_func_re: self.blocked_func_re.clone(),
            blocked_domain_re: self.blocked_domain_re.clone(),
            allowed_domain_re: self.allowed_domain_re.clone(),
            sandbox_re: self.sandbox_re.clone(),
            op_count: AtomicUsize::new(self.op_count.load(Ordering::Relaxed)),
            turn_start: Mutex::new(None),
            audit_path: self.audit_path.clone(),
        }
    }
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
            .filter_map(|p| Regex::new(&format!("^{}", regex::escape(p))).ok())
            .collect();

        let blocked_func_re: Vec<Regex> = config
            .blocked_functions
            .iter()
            .filter_map(|f| {
                Regex::new(&format!(r#"\(\s*{}\b"#, regex::escape(f))).ok()
            })
            .collect();

        let blocked_domain_re: Vec<Regex> = config
            .blocked_domains
            .iter()
            .filter_map(|d| Regex::new(&regex::escape(d)).ok())
            .collect();

        let allowed_domain_re: Vec<Regex> = config
            .allowed_domains
            .iter()
            .filter_map(|d| Regex::new(&regex::escape(d)).ok())
            .collect();

        let sandbox_re: Vec<Regex> = config
            .sandbox_paths
            .iter()
            .filter_map(|p| Regex::new(&format!("^{}", regex::escape(p))).ok())
            .collect();

        let audit_path = config.audit_log.as_deref().map(PathBuf::from);

        Self {
            config,
            blocked_cmd_re,
            blocked_path_re,
            blocked_func_re,
            blocked_domain_re,
            allowed_domain_re,
            sandbox_re,
            op_count: AtomicUsize::new(0),
            turn_start: Mutex::new(None),
            audit_path,
        }
    }

    pub fn start_turn(&self) {
        self.op_count.store(0, Ordering::Relaxed);
        if let Ok(mut start) = self.turn_start.lock() {
            *start = Some(Instant::now());
        }
    }

    pub fn check_code(&self, code: &str) -> Result<(), String> {
        if self.config.mode == SecurityMode::Off {
            return Ok(());
        }

        let ops = self.op_count.fetch_add(1, Ordering::Relaxed);
        if ops >= self.config.max_ops_per_turn {
            return Err(format!(
                "security: rate limit exceeded (max {} ops per turn)",
                self.config.max_ops_per_turn
            ));
        }

        for re in &self.blocked_cmd_re {
            if re.is_match(code) {
                return Err(format!(
                    "security: blocked command pattern '{}'",
                    re.as_str()
                ));
            }
        }

        if self.config.mode == SecurityMode::Strict {
            for re in &self.blocked_func_re {
                if re.is_match(code) {
                    return Err(format!(
                        "security: blocked function '{}'",
                        re.as_str().trim()
                    ));
                }
            }

            if self.config.require_confirm_sudo && re_contains(code, "sudo") {
                return Err("security: sudo blocked in strict mode".to_string());
            }

            if self.config.require_confirm_eval && re_contains(code, "(eval ") {
                return Err("security: eval blocked in strict mode".to_string());
            }
        }

        if !self.config.allow_network && re_contains(code, "http") {
            return Err("security: network access disabled".to_string());
        }

        if !self.config.blocked_domains.is_empty() {
            for re in &self.blocked_domain_re {
                if re.is_match(code) {
                    return Err(format!(
                        "security: blocked domain '{}'",
                        re.as_str()
                    ));
                }
            }
        }

        if !self.config.allowed_domains.is_empty() {
            if let Some(url) = extract_url(code) {
                let allowed = self.allowed_domain_re.iter().any(|re| re.is_match(&url));
                if !allowed {
                    return Err(format!("security: domain not in allowlist: {}", url));
                }
            }
        }

        Ok(())
    }

    pub fn confirm_dangerous(&self, code: &str) -> Result<(), String> {
        if self.config.mode == SecurityMode::Off {
            return Ok(());
        }

        let mut reasons: Vec<String> = Vec::new();

        if self.config.require_confirm_rm && re_contains(code, "rm ") {
            reasons.push("file deletion (rm)".to_string());
        }

        if self.config.require_confirm_sudo && re_contains(code, "sudo") {
            reasons.push("sudo".to_string());
        }

        if self.config.require_confirm_eval && re_contains(code, "(eval ") {
            reasons.push("eval".to_string());
        }

        if self.config.require_confirm_write_system {
            for re in &self.blocked_path_re {
                if re.is_match(code) {
                    reasons.push("writing to system path".to_string());
                    break;
                }
            }
        }

        if self.config.mode == SecurityMode::Confirm {
            for re in &self.blocked_func_re {
                if re.is_match(code) {
                    reasons.push("blocked function".to_string());
                    break;
                }
            }
        }

        if !self.config.sandbox_paths.is_empty() && is_write_op(code) {
            if let Some(path) = extract_path(code) {
                let in_sandbox = self.sandbox_re.iter().any(|re| re.is_match(&path));
                if !in_sandbox {
                    reasons.push(format!("path outside sandbox: {}", path));
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
            self.audit_log(code, &reason, true);
            Ok(())
        } else {
            self.audit_log(code, &reason, false);
            Err(format!("security: blocked by user ({})", reason))
        }
    }

    pub fn check_output(&self, output: &str) -> String {
        if output.len() > self.config.max_output_bytes {
            let truncated = &output[..self.config.max_output_bytes];
            format!(
                "{}\n\n[output truncated: {} bytes limited to {} bytes]",
                truncated,
                output.len(),
                self.config.max_output_bytes
            )
        } else {
            output.to_string()
        }
    }

    #[allow(dead_code)]
    pub fn check_exec_timeout(&self) -> u64 {
        self.config.exec_timeout_secs
    }

    fn audit_log(&self, code: &str, reason: &str, allowed: bool) {
        if let Some(ref path) = self.audit_path {
            let entry = format!(
                "[{}] {} | {} | {}\n",
                chrono_now(),
                if allowed { "ALLOWED" } else { "BLOCKED" },
                reason,
                code.trim()
            );
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = file.write_all(entry.as_bytes());
            }
        }
    }
}

fn re_contains(text: &str, pattern: &str) -> bool {
    text.to_lowercase().contains(&pattern.to_lowercase())
}

fn extract_url(code: &str) -> Option<String> {
    let re = Regex::new(r#""(https?://[^"]+)""#).ok()?;
    re.captures(code).and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn extract_path(code: &str) -> Option<String> {
    let re = Regex::new(r#""([^"]+)""#).ok()?;
    re.captures(code).and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn is_write_op(code: &str) -> bool {
    let write_funcs = ["write", "write-range", "append", "insert-at", "remove-range", "rm", "delete", "mkdir", "cp", "copy", "mv", "move", "touch"];
    let lower = code.to_lowercase();
    write_funcs.iter().any(|f| lower.contains(&format!("({}", f)))
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}
