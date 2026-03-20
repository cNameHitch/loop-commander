import SwiftUI

struct MetricsBarView: View {
    let metrics: DashboardMetrics
    let isConnected: Bool
    var taskStatuses: [TaskStatus] = []

    /// Count tasks that are actively scheduled (active or currently running)
    private var activeTaskCount: Int {
        if taskStatuses.isEmpty {
            return metrics.activeTasks
        }
        return taskStatuses.filter { $0 == .active || $0 == .running }.count
    }

    private let columns = [
        GridItem(.adaptive(minimum: 120), spacing: 10)
    ]

    var body: some View {
        LazyVGrid(columns: columns, spacing: 12) {
            MetricCard(
                label: "Active Tasks",
                value: "\(activeTaskCount)",
                sub: "\(metrics.totalTasks) total",
                accent: .lcAccent
            )
            MetricCard(
                label: "Total Runs",
                value: formatRunCount(metrics.totalRuns),
                sub: "all time"
            )
            MetricCard(
                label: "Success Rate",
                value: "\(Int(metrics.overallSuccessRate))%",
                sub: "across all tasks",
                accent: metrics.totalRuns == 0 ? .lcTextPrimary : (metrics.overallSuccessRate >= 95 ? .lcGreen : (metrics.overallSuccessRate >= 80 ? .lcAmber : .lcRed))
            )
            MetricCard(
                label: "Total Spend",
                value: formatCost(metrics.totalSpend),
                sub: "API costs"
            )

            // Sparkline chart card (if cost trend data available)
            if let costTrend = metrics.costTrend, !costTrend.isEmpty {
                SparklineChart(data: costTrend)
            }

            MetricCard(
                label: "Daemon",
                value: isConnected ? "UP" : "DOWN",
                sub: isConnected
                    ? (metrics.daemonPID.map { "launchd \u{00B7} PID \($0)" } ?? "launchd")
                    : "not running",
                accent: isConnected ? .lcGreen : .lcRed
            )
        }
        .padding(.vertical, 16)
        .padding(.horizontal, 20)
    }

    private static let runCountFormatter: NumberFormatter = {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        return f
    }()

    private func formatRunCount(_ count: Int) -> String {
        Self.runCountFormatter.string(from: NSNumber(value: count)) ?? "\(count)"
    }
}
