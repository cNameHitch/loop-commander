import Foundation

/// Dashboard aggregate metrics, mirroring Rust `DashboardMetrics`.
struct DashboardMetrics: Codable {
    let totalTasks: Int
    let activeTasks: Int
    let totalRuns: Int
    let overallSuccessRate: Double
    let totalSpend: Double
    let tasks: [TaskMetrics]
    var costTrend: [DailyCost]?
    var daemonPID: Int?

    enum CodingKeys: String, CodingKey {
        case totalTasks = "total_tasks"
        case activeTasks = "active_tasks"
        case totalRuns = "total_runs"
        case overallSuccessRate = "overall_success_rate"
        case totalSpend = "total_spend"
        case tasks
        case costTrend = "cost_trend"
        case daemonPID = "daemon_pid"
    }

    /// Empty metrics for initial state
    static let empty = DashboardMetrics(
        totalTasks: 0,
        activeTasks: 0,
        totalRuns: 0,
        overallSuccessRate: 0,
        totalSpend: 0,
        tasks: [],
        costTrend: nil,
        daemonPID: nil
    )
}

/// Per-task metrics, mirroring Rust `TaskMetrics`.
struct TaskMetrics: Codable, Identifiable {
    var id: String { taskId }
    let taskId: String
    let totalRuns: Int
    let successCount: Int
    let failCount: Int
    let totalCost: Double
    let totalTokens: Int
    let avgDurationSecs: Double
    let lastRun: String?

    enum CodingKeys: String, CodingKey {
        case taskId = "task_id"
        case totalRuns = "total_runs"
        case successCount = "success_count"
        case failCount = "fail_count"
        case totalCost = "total_cost"
        case totalTokens = "total_tokens"
        case avgDurationSecs = "avg_duration_secs"
        case lastRun = "last_run"
    }
}

/// Daily cost data point for sparkline chart.
struct DailyCost: Codable, Identifiable {
    var id: String { date }
    let date: String
    let totalCost: Double
    let runCount: Int

    enum CodingKeys: String, CodingKey {
        case date
        case totalCost = "total_cost"
        case runCount = "run_count"
    }
}
