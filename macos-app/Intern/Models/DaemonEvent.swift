import Foundation

/// Events pushed from the daemon to subscribed clients.
/// Mirroring Rust `DaemonEvent` tagged enum with `#[serde(tag = "type", content = "data")]`.
enum DaemonEvent: Codable {
    case taskStarted(taskId: String, taskName: String)
    case taskCompleted(taskId: String, taskName: String, durationSecs: Int, costUsd: Double?)
    case taskFailed(taskId: String, taskName: String, exitCode: Int, summary: String)
    case taskStatusChanged(taskId: String, oldStatus: String, newStatus: String)
    case healthRepair(taskId: String, action: String)
    case budgetExceeded(taskId: String, taskName: String, dailySpend: Double, cap: Double)

    enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    enum DataKeys: String, CodingKey {
        case taskId = "task_id"
        case taskName = "task_name"
        case durationSecs = "duration_secs"
        case costUsd = "cost_usd"
        case exitCode = "exit_code"
        case summary
        case oldStatus = "old_status"
        case newStatus = "new_status"
        case action
        case dailySpend = "daily_spend"
        case cap
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        let dataContainer = try container.nestedContainer(keyedBy: DataKeys.self, forKey: .data)

        switch type {
        case "TaskStarted":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let taskName = try dataContainer.decode(String.self, forKey: .taskName)
            self = .taskStarted(taskId: taskId, taskName: taskName)
        case "TaskCompleted":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let taskName = try dataContainer.decode(String.self, forKey: .taskName)
            let durationSecs = try dataContainer.decode(Int.self, forKey: .durationSecs)
            let costUsd = try dataContainer.decodeIfPresent(Double.self, forKey: .costUsd)
            self = .taskCompleted(taskId: taskId, taskName: taskName, durationSecs: durationSecs, costUsd: costUsd)
        case "TaskFailed":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let taskName = try dataContainer.decode(String.self, forKey: .taskName)
            let exitCode = try dataContainer.decode(Int.self, forKey: .exitCode)
            let summary = try dataContainer.decode(String.self, forKey: .summary)
            self = .taskFailed(taskId: taskId, taskName: taskName, exitCode: exitCode, summary: summary)
        case "TaskStatusChanged":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let oldStatus = try dataContainer.decode(String.self, forKey: .oldStatus)
            let newStatus = try dataContainer.decode(String.self, forKey: .newStatus)
            self = .taskStatusChanged(taskId: taskId, oldStatus: oldStatus, newStatus: newStatus)
        case "HealthRepair":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let action = try dataContainer.decode(String.self, forKey: .action)
            self = .healthRepair(taskId: taskId, action: action)
        case "BudgetExceeded":
            let taskId = try dataContainer.decode(String.self, forKey: .taskId)
            let taskName = try dataContainer.decode(String.self, forKey: .taskName)
            let dailySpend = try dataContainer.decode(Double.self, forKey: .dailySpend)
            let cap = try dataContainer.decode(Double.self, forKey: .cap)
            self = .budgetExceeded(taskId: taskId, taskName: taskName, dailySpend: dailySpend, cap: cap)
        default:
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: container.codingPath,
                    debugDescription: "Unknown event type: \(type)"
                )
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        var dataContainer = container.nestedContainer(keyedBy: DataKeys.self, forKey: .data)

        switch self {
        case .taskStarted(let taskId, let taskName):
            try container.encode("TaskStarted", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(taskName, forKey: .taskName)
        case .taskCompleted(let taskId, let taskName, let durationSecs, let costUsd):
            try container.encode("TaskCompleted", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(taskName, forKey: .taskName)
            try dataContainer.encode(durationSecs, forKey: .durationSecs)
            try dataContainer.encodeIfPresent(costUsd, forKey: .costUsd)
        case .taskFailed(let taskId, let taskName, let exitCode, let summary):
            try container.encode("TaskFailed", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(taskName, forKey: .taskName)
            try dataContainer.encode(exitCode, forKey: .exitCode)
            try dataContainer.encode(summary, forKey: .summary)
        case .taskStatusChanged(let taskId, let oldStatus, let newStatus):
            try container.encode("TaskStatusChanged", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(oldStatus, forKey: .oldStatus)
            try dataContainer.encode(newStatus, forKey: .newStatus)
        case .healthRepair(let taskId, let action):
            try container.encode("HealthRepair", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(action, forKey: .action)
        case .budgetExceeded(let taskId, let taskName, let dailySpend, let cap):
            try container.encode("BudgetExceeded", forKey: .type)
            try dataContainer.encode(taskId, forKey: .taskId)
            try dataContainer.encode(taskName, forKey: .taskName)
            try dataContainer.encode(dailySpend, forKey: .dailySpend)
            try dataContainer.encode(cap, forKey: .cap)
        }
    }
}
