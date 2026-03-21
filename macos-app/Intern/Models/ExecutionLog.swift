import Foundation

/// Execution log entry, mirroring the Rust `ExecutionLog` struct.
struct ExecutionLog: Codable, Identifiable, Hashable {
    let id: Int
    let taskId: String
    let taskName: String
    let startedAt: String
    let finishedAt: String
    let durationSecs: Int
    let exitCode: Int
    let status: String
    let stdout: String
    let stderr: String
    let tokensUsed: Int?
    let costUsd: Double?
    let summary: String

    enum CodingKeys: String, CodingKey {
        case id
        case taskId = "task_id"
        case taskName = "task_name"
        case startedAt = "started_at"
        case finishedAt = "finished_at"
        case durationSecs = "duration_secs"
        case exitCode = "exit_code"
        case status, stdout, stderr
        case tokensUsed = "tokens_used"
        case costUsd = "cost_usd"
        case summary
    }

    /// Parsed timestamp for display
    var timestamp: Date {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: startedAt) {
            return date
        }
        // Try without fractional seconds
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: startedAt) ?? Date()
    }

    /// Combined output for display in expanded view
    var output: String {
        if !stdout.isEmpty && !stderr.isEmpty {
            return "STDOUT:\n\(stdout)\n\nSTDERR:\n\(stderr)"
        }
        return stdout.isEmpty ? stderr : stdout
    }

    /// Whether this execution was successful
    var isSuccess: Bool {
        status == "success"
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }

    static func == (lhs: ExecutionLog, rhs: ExecutionLog) -> Bool {
        lhs.id == rhs.id
    }
}

/// Log query parameters sent to the daemon
struct LogQuery: Codable {
    var taskId: String?
    var status: String?
    var limit: Int?
    var offset: Int?
    var search: String?

    enum CodingKeys: String, CodingKey {
        case taskId = "task_id"
        case status, limit, offset, search
    }
}

/// Filter options for log view
enum LogFilter: String, CaseIterable {
    case all = "all"
    case success = "success"
    case error = "failed"

    var displayName: String {
        switch self {
        case .all:     return "All"
        case .success: return "Success"
        case .error:   return "Error"
        }
    }
}
