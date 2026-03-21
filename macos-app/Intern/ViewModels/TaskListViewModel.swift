import Foundation
import Combine

/// ViewModel for the task list view.
@MainActor
class TaskListViewModel: ObservableObject {
    @Published var tasks: [INTask] = []
    @Published var isLoading: Bool = false
    @Published var error: String?
    @Published var templates: [TaskTemplate] = []

    private var client: DaemonClient?

    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    func loadTasks() async {
        guard let client = client else { return }
        isLoading = true
        error = nil

        do {
            var loadedTasks = try await client.listTasks()

            // Enrich tasks with metrics
            do {
                let metrics: DashboardMetrics = try await client.getDashboardMetrics()
                for i in loadedTasks.indices {
                    if let taskMetrics = metrics.tasks.first(where: { $0.taskId == loadedTasks[i].id }) {
                        loadedTasks[i].runCount = taskMetrics.totalRuns
                        loadedTasks[i].successCount = taskMetrics.successCount
                        loadedTasks[i].totalCost = taskMetrics.totalCost
                        if let lastRun = taskMetrics.lastRun {
                            loadedTasks[i].lastRun = parseISO8601(lastRun)
                        }
                    }
                }
            } catch {
                // Metrics enrichment is best-effort
            }

            tasks = loadedTasks
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func createTask(_ draft: INTaskDraft) async -> INTask? {
        guard let client = client else { return nil }
        do {
            let params = draft.toCreateInput()
            let task = try await client.createTask(params)
            await loadTasks()
            return task
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    func updateTask(id: String, draft: INTaskDraft) async -> INTask? {
        guard let client = client else { return nil }
        do {
            let params = draft.toUpdateInput(id: id)
            let task = try await client.updateTask(params)
            await loadTasks()
            return task
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }

    func deleteTask(_ id: String) async {
        guard let client = client else { return }
        do {
            try await client.deleteTask(id)
            tasks.removeAll { $0.id == id }
        } catch {
            self.error = error.localizedDescription
        }
    }

    func pauseTask(_ id: String) async {
        guard let client = client else { return }
        do {
            let updated = try await client.pauseTask(id)
            if let idx = tasks.firstIndex(where: { $0.id == id }) {
                tasks[idx] = updated
            }
        } catch {
            self.error = error.localizedDescription
        }
    }

    func resumeTask(_ id: String) async {
        guard let client = client else { return }
        do {
            let updated = try await client.resumeTask(id)
            if let idx = tasks.firstIndex(where: { $0.id == id }) {
                tasks[idx] = updated
            }
        } catch {
            self.error = error.localizedDescription
        }
    }

    func loadTemplates() async {
        guard let client = client else { return }
        do {
            templates = try await client.getTemplates()
        } catch {
            // Templates are optional
        }
    }
}
