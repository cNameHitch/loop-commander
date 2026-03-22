import Foundation

/// ViewModel for the task detail view.
@MainActor
class TaskDetailViewModel: ObservableObject {
    @Published var task: INTask?
    @Published var logs: [ExecutionLog] = []
    @Published var isLoading: Bool = false
    @Published var error: String?
    @Published var dryRunResult: DryRunResult?
    @Published var showDryRun: Bool = false

    private var client: DaemonClient?
    private var pollTask: Task<Void, Never>?
    private var currentTaskId: String?

    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    func loadTask(_ id: String) async {
        // Cancel poll if switching to a different task.
        if id != currentTaskId {
            pollTask?.cancel()
            pollTask = nil
            currentTaskId = id
        }
        guard let client = client else { return }
        isLoading = true
        error = nil

        do {
            task = try await client.getTask(id)

            // Load execution logs for this task
            let query = LogQuery(taskId: id, limit: 50)
            logs = try await client.queryLogs(query)

            // Enrich task with metrics
            if var t = task {
                do {
                    let metrics: DashboardMetrics = try await client.getDashboardMetrics()
                    if let taskMetrics = metrics.tasks.first(where: { $0.taskId == id }) {
                        t.runCount = taskMetrics.totalRuns
                        t.successCount = taskMetrics.successCount
                        t.totalCost = taskMetrics.totalCost
                        if let lastRun = taskMetrics.lastRun {
                            t.lastRun = parseISO8601(lastRun)
                        }
                    }
                    task = t
                } catch {
                    // Metrics enrichment is best-effort
                }
            }
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func runNow() async {
        guard let client = client, let task = task else { return }
        do {
            try await client.runTaskNow(task.id)
            // Refresh quickly to show Running status
            try? await Task.sleep(nanoseconds: 500_000_000)
            await loadTask(task.id)
            // Poll until no longer running
            pollWhileRunning(task.id)
        } catch {
            self.error = error.localizedDescription
        }
    }

    func stopTask() async {
        guard let client = client, let task = task else { return }
        do {
            try await client.stopTask(task.id)
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            await loadTask(task.id)
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func pollWhileRunning(_ id: String) {
        pollTask?.cancel()
        pollTask = Task {
            while !Task.isCancelled, let t = task, t.status == .running {
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                guard !Task.isCancelled else { break }
                await loadTask(id)
            }
        }
    }

    func dryRun() async {
        guard let client = client, let task = task else { return }
        do {
            dryRunResult = try await client.dryRunTask(task.id)
            showDryRun = true
        } catch {
            self.error = error.localizedDescription
        }
    }

    func pauseTask() async {
        guard let client = client, let task = task else { return }
        do {
            self.task = try await client.pauseTask(task.id)
        } catch {
            self.error = error.localizedDescription
        }
    }

    func resumeTask() async {
        guard let client = client, let task = task else { return }
        do {
            self.task = try await client.resumeTask(task.id)
        } catch {
            self.error = error.localizedDescription
        }
    }

    func deleteTask() async -> Bool {
        guard let client = client, let task = task else { return false }
        do {
            try await client.deleteTask(task.id)
            return true
        } catch {
            self.error = error.localizedDescription
            return false
        }
    }

    func exportTask() async -> TaskExport? {
        guard let client = client, let task = task else { return nil }
        do {
            return try await client.exportTask(task.id)
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }
}
