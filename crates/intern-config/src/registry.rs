//! Agent registry: fetch, parse, cache, and load from the
//! awesome-claude-code-subagents GitHub repository.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use intern_core::prompt::{AgentEntry, AgentRegistry};
use intern_core::InternError;

/// URL for the raw README.md of the awesome-claude-code-subagents repository.
const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/anthropics/awesome-claude-code-subagents/main/README.md";

/// Maximum age in hours before the cache is considered stale.
const MAX_CACHE_AGE_HOURS: i64 = 24;

/// Maximum number of previous prompt versions to keep.
const MAX_PROMPT_VERSIONS: usize = 3;

/// Manages the agent registry cache at `~/.intern/agent-registry.json`.
pub struct RegistryManager {
    cache_path: PathBuf,
    prompts_dir: PathBuf,
}

impl RegistryManager {
    /// Create a new `RegistryManager` with paths derived from the root directory.
    pub fn new(root: &Path) -> Self {
        Self {
            cache_path: root.join("agent-registry.json"),
            prompts_dir: root.join("prompts"),
        }
    }

    /// Load the cached registry from disk. Falls back to built-in agents if no cache exists.
    pub fn load_cache(&self) -> AgentRegistry {
        match std::fs::read_to_string(&self.cache_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse agent registry cache: {e}");
                Self::builtin_registry()
            }),
            Err(_) => Self::builtin_registry(),
        }
    }

    /// Return the built-in agent registry that ships with Intern.
    fn builtin_registry() -> AgentRegistry {
        AgentRegistry {
            fetched_at: Utc::now(),
            agents: intern_core::builtin_agents::builtin_agents(),
        }
    }

    /// Save the registry to the cache file using atomic writes.
    pub fn save_cache(&self, registry: &AgentRegistry) -> Result<(), InternError> {
        let content = serde_json::to_string_pretty(registry)
            .map_err(|e| InternError::Config(format!("Failed to serialize registry: {e}")))?;
        atomic_write_json(&self.cache_path, content.as_bytes())
    }

    /// Check whether the cached registry is stale (older than 24 hours).
    pub fn is_cache_stale(&self) -> bool {
        self.load_cache().is_stale(MAX_CACHE_AGE_HOURS)
    }

    /// Fetch the README from GitHub, parse it into agent entries, and update the cache.
    ///
    /// Returns the new registry on success. On fetch failure, returns the existing
    /// cache silently (never blocks on network errors).
    pub async fn refresh(&self) -> Result<AgentRegistry, InternError> {
        let readme = match fetch_readme().await {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!("Failed to fetch agent registry from GitHub: {e}");
                return Ok(self.load_cache());
            }
        };

        let agents = parse_agent_readme(&readme);
        let registry = AgentRegistry {
            fetched_at: Utc::now(),
            agents,
        };

        self.save_cache(&registry)?;
        tracing::info!(
            "Refreshed agent registry: {} agents cached",
            registry.agents.len()
        );
        Ok(registry)
    }

    /// Ensure the prompts directory exists.
    pub fn ensure_prompts_dir(&self) -> Result<(), InternError> {
        std::fs::create_dir_all(&self.prompts_dir).map_err(|e| {
            InternError::Config(format!(
                "Failed to create prompts directory {}: {e}",
                self.prompts_dir.display()
            ))
        })
    }

    /// Return the path for a prompt file given a task ID.
    pub fn prompt_path(&self, task_id: &str) -> PathBuf {
        self.prompts_dir.join(format!("{task_id}.md"))
    }

    /// Write a generated prompt to disk using atomic writes.
    ///
    /// If a previous version exists, it is preserved as `<task-id>.v<n>.md`.
    /// At most `MAX_PROMPT_VERSIONS` previous versions are kept.
    pub fn save_prompt(&self, task_id: &str, content: &str) -> Result<PathBuf, InternError> {
        self.ensure_prompts_dir()?;

        let path = self.prompt_path(task_id);

        // Version the existing file if it exists.
        if path.exists() {
            self.version_existing_prompt(task_id)?;
        }

        atomic_write_md(&path, content.as_bytes())?;
        tracing::debug!("Saved prompt to {}", path.display());
        Ok(path)
    }

    /// Rotate existing prompt versions, keeping at most MAX_PROMPT_VERSIONS.
    fn version_existing_prompt(&self, task_id: &str) -> Result<(), InternError> {
        // Find the next version number
        let mut next_version = 1u32;
        loop {
            let versioned = self
                .prompts_dir
                .join(format!("{task_id}.v{next_version}.md"));
            if !versioned.exists() {
                break;
            }
            next_version += 1;
        }

        // Rename current to versioned
        let current = self.prompt_path(task_id);
        let versioned = self
            .prompts_dir
            .join(format!("{task_id}.v{next_version}.md"));
        std::fs::rename(&current, &versioned).map_err(|e| {
            InternError::Config(format!(
                "Failed to version prompt {}: {e}",
                current.display()
            ))
        })?;

        // Delete the oldest if we exceed the limit
        if next_version as usize > MAX_PROMPT_VERSIONS {
            let oldest_version = next_version - MAX_PROMPT_VERSIONS as u32;
            let oldest = self
                .prompts_dir
                .join(format!("{task_id}.v{oldest_version}.md"));
            if oldest.exists() {
                let _ = std::fs::remove_file(&oldest);
            }
        }

        Ok(())
    }

    /// Return the prompts directory path.
    pub fn prompts_dir(&self) -> &Path {
        &self.prompts_dir
    }
}

// ── HTTP Fetch ──────────────────────────────────────────

/// Fetch the README.md content from GitHub.
async fn fetch_readme() -> Result<String, String> {
    // Use a subprocess curl call to avoid adding reqwest as a dependency.
    // This keeps the binary lean and avoids OpenSSL/TLS linking issues.
    let output = tokio::process::Command::new("curl")
        .args(["-sfL", "--max-time", "10", REGISTRY_URL])
        .output()
        .await
        .map_err(|e| format!("Failed to spawn curl: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 in response: {e}"))
}

// ── README Parser ───────────────────────────────────────

/// Parse the awesome-claude-code-subagents README.md into agent entries.
///
/// The README uses a consistent structure where agents are listed under
/// category headings. The parser handles two common formats:
///
/// 1. Table format: `| slug | Name | Description |`
/// 2. List format: `- **Name** (slug): Description`
/// 3. Section format: `## Agent Name` with description paragraph
pub fn parse_agent_readme(readme: &str) -> Vec<AgentEntry> {
    let mut agents = Vec::new();
    let mut current_category = String::new();
    let mut skip_section = false;

    let lines: Vec<&str> = readme.lines().collect();
    let mut i = 0;

    const SKIP_SECTIONS: &[&str] = &[
        "table of contents",
        "contributing",
        "license",
        "overview",
        "introduction",
        "getting started",
        "installation",
        "usage",
        "about",
        "contents",
    ];

    while i < lines.len() {
        let line = lines[i].trim();

        // Track category from ## headings
        if let Some(heading) = line.strip_prefix("## ") {
            current_category = heading.trim().to_lowercase();
            skip_section = SKIP_SECTIONS.contains(&current_category.as_str());
            if skip_section {
                i += 1;
                continue;
            }
        }

        if skip_section {
            i += 1;
            continue;
        }

        // Format 1: Table rows (| slug | name | description |)
        if line.starts_with('|') && !line.contains("---") {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            // Need at least slug and description columns (filtering header rows)
            if cols.len() >= 4 {
                let col1 = cols[1];
                let col2 = cols[2];
                let col3 = cols[3];

                // Skip header row
                if col1 != "Name"
                    && col1 != "Agent"
                    && col1 != "Slug"
                    && !col1.is_empty()
                    && !col1.contains("---")
                {
                    let slug = slugify(col1);
                    if !slug.is_empty() {
                        agents.push(AgentEntry {
                            slug,
                            name: col1.to_string(),
                            description: if col3.is_empty() {
                                col2.to_string()
                            } else {
                                col3.to_string()
                            },
                            category: current_category.clone(),
                        });
                    }
                }
            }
        }

        // Format 2: List items with bold name
        // - **Name** or - **Name**: Description or - [**Name**](link): Description
        if line.starts_with("- ") || line.starts_with("* ") {
            let item = &line[2..];

            // Try to extract name and description from bold pattern
            if let Some(entry) = parse_list_item(item, &current_category) {
                agents.push(entry);
            }
        }

        i += 1;
    }

    // Deduplicate by slug
    agents.sort_by(|a, b| a.slug.cmp(&b.slug));
    agents.dedup_by(|a, b| a.slug == b.slug);

    agents
}

/// Parse a list item into an AgentEntry.
///
/// Handles patterns like:
/// - `**Name**: Description`
/// - `[**Name**](url): Description`
/// - `**Name** - Description`
fn parse_list_item(item: &str, category: &str) -> Option<AgentEntry> {
    // Strip markdown link wrapper if present: [**Name**](url) -> **Name**
    let item = if item.starts_with('[') {
        if let Some(close_bracket) = item.find("](") {
            let inner = &item[1..close_bracket];
            if let Some(close_paren) = item[close_bracket..].find(')') {
                let rest = &item[close_bracket + close_paren + 1..];
                format!("{inner}{rest}").leak() // Safe in this context: small allocations during parsing
            } else {
                item
            }
        } else {
            item
        }
    } else {
        item
    };

    // Extract bold name: **Name**
    let bold_start = item.find("**")?;
    let after_bold = &item[bold_start + 2..];
    let bold_end = after_bold.find("**")?;
    let name = after_bold[..bold_end].trim().to_string();

    if name.is_empty() {
        return None;
    }

    // Extract description (after ** and separator)
    let after_name = &after_bold[bold_end + 2..].trim();
    let description = after_name
        .strip_prefix(':')
        .or_else(|| after_name.strip_prefix('-'))
        .or_else(|| after_name.strip_prefix('–'))
        .unwrap_or(after_name)
        .trim()
        .to_string();

    let slug = slugify(&name);
    if slug.is_empty() {
        return None;
    }

    Some(AgentEntry {
        slug,
        name,
        description,
        category: category.to_string(),
    })
}

/// Convert a name to a kebab-case slug.
fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Atomic Writes ───────────────────────────────────────

/// Atomic write for JSON files (temp + fsync + rename).
fn atomic_write_json(path: &Path, content: &[u8]) -> Result<(), InternError> {
    let tmp_path = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
        InternError::Config(format!(
            "Failed to create temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.write_all(content).map_err(|e| {
        InternError::Config(format!(
            "Failed to write temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.sync_all().map_err(|e| {
        InternError::Config(format!(
            "Failed to fsync temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        InternError::Config(format!(
            "Failed to rename {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Atomic write for Markdown prompt files (temp + fsync + rename).
fn atomic_write_md(path: &Path, content: &[u8]) -> Result<(), InternError> {
    let tmp_path = path.with_extension("md.tmp");
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| {
        InternError::Config(format!(
            "Failed to create temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.write_all(content).map_err(|e| {
        InternError::Config(format!(
            "Failed to write temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    file.sync_all().map_err(|e| {
        InternError::Config(format!(
            "Failed to fsync temp file {}: {e}",
            tmp_path.display()
        ))
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        InternError::Config(format!(
            "Failed to rename {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn slugify_name() {
        assert_eq!(slugify("Security Auditor"), "security-auditor");
        assert_eq!(slugify("Code Reviewer"), "code-reviewer");
        assert_eq!(slugify("ML/AI Engineer"), "ml-ai-engineer");
        assert_eq!(slugify("test"), "test");
    }

    #[test]
    fn parse_list_item_bold_colon() {
        let entry = parse_list_item("**Security Auditor**: Audits code for security", "security");
        let entry = entry.unwrap();
        assert_eq!(entry.slug, "security-auditor");
        assert_eq!(entry.name, "Security Auditor");
        assert_eq!(entry.description, "Audits code for security");
        assert_eq!(entry.category, "security");
    }

    #[test]
    fn parse_list_item_bold_dash() {
        let entry = parse_list_item("**Code Reviewer** - Reviews code quality", "quality");
        let entry = entry.unwrap();
        assert_eq!(entry.slug, "code-reviewer");
        assert_eq!(entry.description, "Reviews code quality");
    }

    #[test]
    fn parse_readme_with_list_items() {
        let readme = r#"# Awesome Agents

## Security

- **Security Auditor**: Performs security audits on codebases
- **Penetration Tester**: Tests systems for vulnerabilities

## Quality

- **Code Reviewer**: Reviews code for quality and correctness

## Contributing

Please submit a PR.
"#;

        let agents = parse_agent_readme(readme);
        assert_eq!(agents.len(), 3);

        let auditor = agents
            .iter()
            .find(|a| a.slug == "security-auditor")
            .unwrap();
        assert_eq!(auditor.category, "security");
        assert_eq!(auditor.description, "Performs security audits on codebases");

        let reviewer = agents.iter().find(|a| a.slug == "code-reviewer").unwrap();
        assert_eq!(reviewer.category, "quality");
    }

    #[test]
    fn parse_readme_with_table() {
        let readme = r#"# Agents

## Available Agents

| Name | Slug | Description |
|------|------|-------------|
| Security Auditor | security-auditor | Audits security |
| Code Reviewer | code-reviewer | Reviews code |
"#;

        let agents = parse_agent_readme(readme);
        assert!(agents.len() >= 2, "Got {} agents", agents.len());
    }

    #[test]
    fn parse_readme_skips_non_agent_sections() {
        let readme = r#"# Agents

## Contributing

- **Not An Agent**: This should be skipped

## License

MIT
"#;

        let agents = parse_agent_readme(readme);
        assert!(agents.is_empty());
    }

    #[test]
    fn registry_manager_cache_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mgr = RegistryManager::new(tmp.path());

        let registry = AgentRegistry {
            fetched_at: Utc::now(),
            agents: vec![AgentEntry {
                slug: "test-agent".into(),
                name: "Test Agent".into(),
                description: "A test agent".into(),
                category: "testing".into(),
            }],
        };

        mgr.save_cache(&registry).unwrap();
        let loaded = mgr.load_cache();
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents[0].slug, "test-agent");
    }

    #[test]
    fn registry_manager_prompt_save_and_version() {
        let tmp = TempDir::new().unwrap();
        let mgr = RegistryManager::new(tmp.path());

        // Save first version
        let path = mgr.save_prompt("lc-test1234", "# Version 1").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Version 1");

        // Save second version — first should be versioned
        let path = mgr.save_prompt("lc-test1234", "# Version 2").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Version 2");

        let v1_path = tmp.path().join("prompts/lc-test1234.v1.md");
        assert!(v1_path.exists());
        assert_eq!(std::fs::read_to_string(&v1_path).unwrap(), "# Version 1");
    }

    #[test]
    fn registry_manager_returns_builtins_when_no_cache() {
        let tmp = TempDir::new().unwrap();
        let mgr = RegistryManager::new(tmp.path());
        let registry = mgr.load_cache();
        // When no cache file exists, the built-in agents are returned
        assert!(
            registry.agents.len() > 100,
            "Expected built-in agents, got {}",
            registry.agents.len()
        );
    }
}
