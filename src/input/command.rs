use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::error::Result;
use crate::models::common::Route;

const MAX_HISTORY_ENTRY_LEN: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Navigate(Route),
    Quit,
    Refresh,
    Help,
    ContextSwitch(String),
    ContextList,
    Unknown(String),
}

/// Single source of truth for command name → (abbreviation, Route) mappings.
/// Both abbreviation map and route map are derived from this table.
struct CommandDef {
    name: &'static str,
    abbreviation: &'static str,
    route: Route,
}

const COMMAND_TABLE: &[CommandDef] = &[
    CommandDef { name: "servers", abbreviation: "srv", route: Route::Servers },
    CommandDef { name: "networks", abbreviation: "net", route: Route::Networks },
    CommandDef { name: "volumes", abbreviation: "vol", route: Route::Volumes },
    CommandDef { name: "floatingip", abbreviation: "fip", route: Route::FloatingIps },
    CommandDef { name: "security-groups", abbreviation: "sec", route: Route::SecurityGroups },
    CommandDef { name: "images", abbreviation: "img", route: Route::Images },
    CommandDef { name: "flavors", abbreviation: "flv", route: Route::Flavors },
    CommandDef { name: "projects", abbreviation: "prj", route: Route::Projects },
    CommandDef { name: "users", abbreviation: "usr", route: Route::Users },
    CommandDef { name: "aggregates", abbreviation: "agg", route: Route::Aggregates },
    CommandDef { name: "hypervisors", abbreviation: "hyp", route: Route::Hypervisors },
    CommandDef { name: "migrations", abbreviation: "mig", route: Route::Migrations },
    CommandDef { name: "snapshots", abbreviation: "snap", route: Route::Snapshots },
    CommandDef { name: "compute-services", abbreviation: "svc", route: Route::ComputeServices },
    CommandDef { name: "agents", abbreviation: "agt", route: Route::Agents },
    CommandDef { name: "usage", abbreviation: "usg", route: Route::Usage },
];

fn build_abbreviations() -> HashMap<String, String> {
    COMMAND_TABLE
        .iter()
        .map(|def| (def.abbreviation.to_string(), def.name.to_string()))
        .collect()
}

fn build_route_map() -> HashMap<String, Route> {
    COMMAND_TABLE
        .iter()
        .map(|def| (def.name.to_string(), def.route))
        .collect()
}

pub struct CommandParser {
    abbreviations: HashMap<String, String>,
    route_map: HashMap<String, Route>,
    history: CommandHistory,
    completions: Vec<String>,
    completion_index: usize,
    last_prefix: Option<String>,
}

impl CommandParser {
    pub fn new(history_path: PathBuf) -> Self {
        Self {
            abbreviations: build_abbreviations(),
            route_map: build_route_map(),
            history: CommandHistory::new(history_path, 50),
            completions: Vec::new(),
            completion_index: 0,
            last_prefix: None,
        }
    }

    /// Parse a command string. Resolves abbreviations first.
    pub fn parse(&mut self, input: &str) -> Command {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Command::Unknown(String::new());
        }

        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let arg = parts.get(1).map(|s| s.trim().to_string());

        // Resolve abbreviation
        let resolved = self
            .abbreviations
            .get(&cmd)
            .cloned()
            .unwrap_or_else(|| cmd.clone());

        // System commands
        match resolved.as_str() {
            "q" | "quit" => return Command::Quit,
            "refresh" => return Command::Refresh,
            "help" => return Command::Help,
            "ctx" => {
                return match arg {
                    Some(cloud) if !cloud.is_empty() => Command::ContextSwitch(cloud),
                    _ => Command::ContextList,
                };
            }
            _ => {}
        }

        // Route navigation
        if let Some(route) = self.route_map.get(&resolved) {
            return Command::Navigate(*route);
        }

        Command::Unknown(trimmed.to_string())
    }

    /// Tab auto-complete. Returns the expanded command name (not the abbreviation).
    /// First Tab: collect matching commands by prefix, return first.
    /// Subsequent Tabs with same prefix: cycle through matches.
    /// If the prefix exactly matches an abbreviation, its expanded form is included.
    pub fn auto_complete(&mut self, prefix: &str) -> Option<String> {
        let prefix_lower = prefix.to_lowercase();

        if self.last_prefix.as_deref() != Some(&prefix_lower) {
            self.completions = self
                .available_commands()
                .into_iter()
                .filter(|cmd| cmd.starts_with(&prefix_lower))
                .collect();
            // Include abbreviation expansions
            for (abbr, full) in &self.abbreviations {
                if abbr.starts_with(&prefix_lower) && !self.completions.contains(full) {
                    self.completions.push(full.clone());
                }
            }
            self.completions.sort();
            self.completions.dedup();
            self.completion_index = 0;
            self.last_prefix = Some(prefix_lower);
        } else if !self.completions.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.completions.len();
        }

        self.completions.get(self.completion_index).cloned()
    }

    pub fn reset_completion(&mut self) {
        self.completions.clear();
        self.completion_index = 0;
        self.last_prefix = None;
    }

    pub fn push_history(&mut self, command: &str) {
        self.history.push(command);
    }

    pub fn history_prev(&mut self) -> Option<&str> {
        self.history.prev()
    }

    pub fn history_next(&mut self) -> Option<&str> {
        self.history.next()
    }

    pub fn history_reset_cursor(&mut self) {
        self.history.reset_cursor();
    }

    pub fn save_history(&self) -> Result<()> {
        self.history.save()
    }

    pub fn load_history(&mut self) -> Result<()> {
        self.history.load()
    }

    /// All valid command names (for auto-complete).
    pub fn available_commands(&self) -> Vec<String> {
        let mut cmds: Vec<String> = self.route_map.keys().cloned().collect();
        cmds.extend(["quit", "refresh", "help", "ctx"].iter().map(|s| s.to_string()));
        cmds.sort();
        cmds
    }
}

// --- CommandHistory ---

struct CommandHistory {
    entries: Vec<String>,
    max_size: usize,
    cursor: Option<usize>,
    file_path: PathBuf,
}

impl CommandHistory {
    fn new(file_path: PathBuf, max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            cursor: None,
            file_path,
        }
    }

    fn push(&mut self, command: &str) {
        let cmd = command.trim().to_string();
        if cmd.is_empty() || cmd.len() > MAX_HISTORY_ENTRY_LEN {
            return;
        }
        self.entries.retain(|e| e != &cmd);
        self.entries.push(cmd);
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            None => self.entries.len().saturating_sub(1),
            Some(0) => 0,
            Some(c) => c - 1,
        };
        self.cursor = Some(idx);
        self.entries.get(idx).map(|s| s.as_str())
    }

    fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(c) => {
                if c + 1 >= self.entries.len() {
                    self.cursor = None;
                    None
                } else {
                    self.cursor = Some(c + 1);
                    self.entries.get(c + 1).map(|s| s.as_str())
                }
            }
        }
    }

    fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::AppError::Other(format!(
                    "Failed to create history directory: {e}"
                ))
            })?;
        }
        let content = self.entries.join("\n");
        fs::write(&self.file_path, content).map_err(|e| {
            crate::error::AppError::Other(format!("Failed to save history: {e}"))
        })
    }

    fn load(&mut self) -> Result<()> {
        match fs::read_to_string(&self.file_path) {
            Ok(content) => {
                self.entries = content
                    .lines()
                    .filter(|l| !l.is_empty() && l.len() <= MAX_HISTORY_ENTRY_LEN)
                    .map(|l| l.to_string())
                    .collect();
                if self.entries.len() > self.max_size {
                    let excess = self.entries.len() - self.max_size;
                    self.entries.drain(..excess);
                }
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(crate::error::AppError::Other(format!(
                "Failed to load history: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn parser() -> CommandParser {
        let dir = TempDir::new().unwrap();
        CommandParser::new(dir.path().join("history"))
    }

    #[test]
    fn test_parse_route_direct() {
        let mut p = parser();
        assert_eq!(p.parse("servers"), Command::Navigate(Route::Servers));
        assert_eq!(p.parse("networks"), Command::Navigate(Route::Networks));
    }

    #[test]
    fn test_parse_abbreviation() {
        let mut p = parser();
        assert_eq!(p.parse("srv"), Command::Navigate(Route::Servers));
        assert_eq!(p.parse("net"), Command::Navigate(Route::Networks));
        assert_eq!(p.parse("vol"), Command::Navigate(Route::Volumes));
    }

    #[test]
    fn test_parse_system_commands() {
        let mut p = parser();
        assert_eq!(p.parse("quit"), Command::Quit);
        assert_eq!(p.parse("q"), Command::Quit);
        assert_eq!(p.parse("refresh"), Command::Refresh);
        assert_eq!(p.parse("help"), Command::Help);
    }

    #[test]
    fn test_parse_context() {
        let mut p = parser();
        assert_eq!(
            p.parse("ctx prod"),
            Command::ContextSwitch("prod".to_string())
        );
        assert_eq!(p.parse("ctx"), Command::ContextList);
    }

    #[test]
    fn test_parse_unknown() {
        let mut p = parser();
        assert_eq!(p.parse("foobar"), Command::Unknown("foobar".to_string()));
    }

    #[test]
    fn test_parse_case_insensitive() {
        let mut p = parser();
        assert_eq!(p.parse("SERVERS"), Command::Navigate(Route::Servers));
        assert_eq!(p.parse("SRV"), Command::Navigate(Route::Servers));
    }

    #[test]
    fn test_auto_complete_prefix() {
        let mut p = parser();
        let result = p.auto_complete("ser");
        assert_eq!(result, Some("servers".to_string()));
    }

    #[test]
    fn test_auto_complete_cycle() {
        let mut p = parser();
        let r1 = p.auto_complete("s");
        assert!(r1.is_some());
        let r2 = p.auto_complete("s");
        assert!(r2.is_some());
        assert_ne!(r1, r2); // should cycle
    }

    #[test]
    fn test_auto_complete_no_match() {
        let mut p = parser();
        assert!(p.auto_complete("zzz").is_none());
    }

    #[test]
    fn test_history_prev_next() {
        let mut p = parser();
        p.push_history("servers");
        p.push_history("networks");
        p.push_history("volumes");

        assert_eq!(p.history_prev(), Some("volumes"));
        assert_eq!(p.history_prev(), Some("networks"));
        assert_eq!(p.history_prev(), Some("servers"));
        assert_eq!(p.history_prev(), Some("servers"));

        assert_eq!(p.history_next(), Some("networks"));
        assert_eq!(p.history_next(), Some("volumes"));
        assert_eq!(p.history_next(), None);
    }

    #[test]
    fn test_history_dedup() {
        let mut p = parser();
        p.push_history("servers");
        p.push_history("networks");
        p.push_history("servers");

        assert_eq!(p.history_prev(), Some("servers"));
        assert_eq!(p.history_prev(), Some("networks"));
    }

    #[test]
    fn test_history_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history");

        let mut p1 = CommandParser::new(path.clone());
        p1.push_history("servers");
        p1.push_history("networks");
        p1.save_history().unwrap();

        let mut p2 = CommandParser::new(path);
        p2.load_history().unwrap();
        assert_eq!(p2.history_prev(), Some("networks"));
        assert_eq!(p2.history_prev(), Some("servers"));
    }

    #[test]
    fn test_history_max_size() {
        let dir = TempDir::new().unwrap();
        let mut p = CommandParser::new(dir.path().join("history"));
        for i in 0..60 {
            p.push_history(&format!("cmd-{i}"));
        }
        // Max is 50, oldest should be evicted
        assert_eq!(p.history_prev(), Some("cmd-59"));
        // Navigate to earliest
        for _ in 0..49 {
            p.history_prev();
        }
        assert_eq!(p.history_prev(), Some("cmd-10")); // cmd-0..cmd-9 evicted
    }

    #[test]
    fn test_history_entry_length_limit() {
        let mut p = parser();
        let long_cmd = "x".repeat(MAX_HISTORY_ENTRY_LEN + 1);
        p.push_history(&long_cmd);
        assert!(p.history_prev().is_none()); // rejected
    }

    #[test]
    fn test_command_table_sync() {
        // Verify every abbreviation maps to a valid route
        let abbr = build_abbreviations();
        let routes = build_route_map();
        for (_abbr_key, full_name) in &abbr {
            assert!(
                routes.contains_key(full_name),
                "abbreviation maps to '{full_name}' but no route defined"
            );
        }
    }
}
