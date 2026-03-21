import Foundation

/// Main task model, mirroring the Rust `Task` struct.
/// Uses snake_case decoding strategy via JSONDecoder configuration.
struct INTask: Codable, Identifiable, Hashable {
    let id: String
    var name: String
    var command: String
    var skill: String?
    var schedule: Schedule
    var scheduleHuman: String
    var workingDir: String
    var envVars: [String: String]
    var maxBudgetPerRun: Double
    var maxTurns: Int?
    var timeoutSecs: Int
    var status: TaskStatus
    var tags: [String]
    var agents: [String]
    let createdAt: String
    var updatedAt: String

    // Computed properties for display (populated from metrics when available)
    var runCount: Int = 0
    var successCount: Int = 0
    var totalCost: Double = 0.0
    var lastRun: Date? = nil

    enum CodingKeys: String, CodingKey {
        case id, name, command, skill, schedule
        case scheduleHuman = "schedule_human"
        case workingDir = "working_dir"
        case envVars = "env_vars"
        case maxBudgetPerRun = "max_budget_per_run"
        case maxTurns = "max_turns"
        case timeoutSecs = "timeout_secs"
        case status, tags, agents
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }

    // Hashable conformance based on id
    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }

    static func == (lhs: INTask, rhs: INTask) -> Bool {
        lhs.id == rhs.id
    }
}

// MARK: - Draft for Editor

/// Mutable draft used in the task editor form.
struct INTaskDraft {
    var name: String = ""
    var command: String = ""
    var skill: String = ""
    var workingDir: String = "~/Documents/Claude Tasks/"
    var schedule: String = "*/15 * * * *"
    var scheduleHuman: String = "Every 15 minutes"
    var maxBudget: Double = 5.0
    var maxTurns: Int? = nil
    var timeoutSecs: Int = 600
    var tags: [String] = []
    var agents: [String] = []
    var envVars: [String: String] = [:]

    /// Create from an existing task for editing
    init(from task: INTask) {
        name = task.name
        command = task.command
        skill = task.skill ?? ""
        workingDir = task.workingDir
        schedule = task.schedule.cronExpression ?? "*/15 * * * *"
        scheduleHuman = task.scheduleHuman
        maxBudget = task.maxBudgetPerRun
        maxTurns = task.maxTurns
        timeoutSecs = task.timeoutSecs
        tags = task.tags
        agents = task.agents
        envVars = task.envVars
    }

    /// Create empty draft for new task
    init() {}

    /// Create from an imported Claude Code command
    init(from command: ClaudeCommand) {
        name = command.name
            .replacingOccurrences(of: ".", with: " ")
            .replacingOccurrences(of: "-", with: " ")
            .capitalized
        // Strip frontmatter from the content to get the raw prompt body.
        // Store as a raw prompt string (not wrapped in `claude -p '...'`).
        // The runner's build_command will automatically wrap non-claude
        // commands with `claude -p <prompt> --output-format json`.
        self.command = Self.stripFrontmatter(command.content)
        skill = "/\(command.name)"
        workingDir = command.projectPath
        schedule = "0 */2 * * *"
        scheduleHuman = "Every 2 hours"
    }

    /// Remove YAML frontmatter (--- ... ---) from markdown content.
    private static func stripFrontmatter(_ content: String) -> String {
        let lines = content.components(separatedBy: .newlines)
        guard lines.first?.trimmingCharacters(in: .whitespaces) == "---" else {
            return content.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        var endIndex = 1
        for i in 1..<lines.count {
            if lines[i].trimmingCharacters(in: .whitespaces) == "---" {
                endIndex = i + 1
                break
            }
        }
        return lines[endIndex...].joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Create from a template
    init(from template: TaskTemplate) {
        name = template.name
        command = template.command
        schedule = template.schedule.cronExpression ?? "*/15 * * * *"
        scheduleHuman = template.scheduleHuman
        maxBudget = template.maxBudgetPerRun
        tags = template.tags
    }

    /// Convert to CreateTaskInput JSON
    func toCreateInput() -> [String: Any] {
        var params: [String: Any] = [
            "name": name,
            "command": command,
            "schedule": ["type": "cron", "expression": schedule],
            "schedule_human": scheduleHuman,
            "working_dir": workingDir,
            "max_budget_per_run": maxBudget,
            "timeout_secs": timeoutSecs,
            "tags": tags,
            "agents": agents,
        ]
        if !skill.isEmpty {
            params["skill"] = skill
        }
        if let turns = maxTurns {
            params["max_turns"] = turns
        }
        if !envVars.isEmpty {
            params["env_vars"] = envVars
        }
        return params
    }

    /// Convert to UpdateTaskInput JSON
    func toUpdateInput(id: String) -> [String: Any] {
        var params: [String: Any] = ["id": id]
        params["name"] = name
        params["command"] = command
        params["schedule"] = ["type": "cron", "expression": schedule] as [String: Any]
        params["schedule_human"] = scheduleHuman
        params["working_dir"] = workingDir
        params["max_budget_per_run"] = maxBudget
        params["timeout_secs"] = timeoutSecs
        params["tags"] = tags
        params["agents"] = agents
        if !skill.isEmpty {
            params["skill"] = skill
        }
        if let turns = maxTurns {
            params["max_turns"] = turns
        }
        if !envVars.isEmpty {
            params["env_vars"] = envVars
        }
        return params
    }
}

// MARK: - Task Template

struct TaskTemplate: Codable, Identifiable {
    var id: String { slug }
    let slug: String
    let name: String
    let description: String
    let command: String
    let schedule: Schedule
    let scheduleHuman: String
    let maxBudgetPerRun: Double
    let tags: [String]

    enum CodingKeys: String, CodingKey {
        case slug, name, description, command, schedule
        case scheduleHuman = "schedule_human"
        case maxBudgetPerRun = "max_budget_per_run"
        case tags
    }
}

// MARK: - Task Export

struct TaskExport: Codable {
    let version: Int
    let name: String
    let command: String
    let skill: String?
    let schedule: Schedule
    let scheduleHuman: String
    let workingDir: String
    let envVars: [String: String]
    let maxBudgetPerRun: Double
    let maxTurns: Int?
    let timeoutSecs: Int
    let tags: [String]

    enum CodingKeys: String, CodingKey {
        case version, name, command, skill, schedule
        case scheduleHuman = "schedule_human"
        case workingDir = "working_dir"
        case envVars = "env_vars"
        case maxBudgetPerRun = "max_budget_per_run"
        case maxTurns = "max_turns"
        case timeoutSecs = "timeout_secs"
        case tags
    }
}

// MARK: - Dry Run Result

struct DryRunResult: Codable {
    let taskId: String
    let taskName: String
    let resolvedCommand: [String]
    let workingDir: String
    let envVars: [String: String]
    let timeoutSecs: Int
    let maxBudgetPerRun: Double
    let dailySpendSoFar: Double
    let wouldBeSkipped: Bool
    let skipReason: String?
    let scheduleHuman: String

    enum CodingKeys: String, CodingKey {
        case taskId = "task_id"
        case taskName = "task_name"
        case resolvedCommand = "resolved_command"
        case workingDir = "working_dir"
        case envVars = "env_vars"
        case timeoutSecs = "timeout_secs"
        case maxBudgetPerRun = "max_budget_per_run"
        case dailySpendSoFar = "daily_spend_so_far"
        case wouldBeSkipped = "would_be_skipped"
        case skipReason = "skip_reason"
        case scheduleHuman = "schedule_human"
    }
}

// MARK: - Daemon Status

struct DaemonStatus: Codable {
    let pid: Int
    let uptime: Int
    let version: String
    let connectedClients: Int
    let claudeAvailable: Bool

    enum CodingKeys: String, CodingKey {
        case pid, uptime, version
        case connectedClients = "connected_clients"
        case claudeAvailable = "claude_available"
    }
}

// MARK: - INTaskDraft Equatable

extension INTaskDraft: Equatable {
    static func == (lhs: INTaskDraft, rhs: INTaskDraft) -> Bool {
        lhs.name == rhs.name &&
        lhs.command == rhs.command &&
        lhs.skill == rhs.skill &&
        lhs.workingDir == rhs.workingDir &&
        lhs.schedule == rhs.schedule &&
        lhs.scheduleHuman == rhs.scheduleHuman &&
        lhs.maxBudget == rhs.maxBudget &&
        lhs.maxTurns == rhs.maxTurns &&
        lhs.timeoutSecs == rhs.timeoutSecs &&
        lhs.tags == rhs.tags &&
        lhs.agents == rhs.agents &&
        lhs.envVars == rhs.envVars
    }
}
