import Foundation

/// ViewModel for the AI prompt optimizer panel.
///
/// Manages execution log loading, the lifecycle of a `prompt.optimize`
/// RPC call, and feedback-driven re-optimization.  Follows the same
/// `@MainActor` / `ObservableObject` / weak-client patterns used throughout
/// the app (mirrors `PromptGeneratorViewModel`).
@MainActor
class PromptOptimizerViewModel: ObservableObject {

    // MARK: - Published State

    /// True while the `logs.query` RPC call is in flight.
    @Published var isLoadingLogs: Bool = false

    /// True while the `prompt.optimize` RPC call is in flight.
    @Published var isOptimizing: Bool = false

    /// The most recent optimization result; nil until the first successful call.
    @Published var result: PromptOptimizationResult? = nil

    /// Non-nil when an error should be surfaced to the user.
    @Published var error: String? = nil

    /// Execution logs fetched for the current task.
    @Published var executionLogs: [ExecutionLog] = []

    /// Free-text feedback used when calling `reoptimize()`.
    @Published var feedbackText: String = ""

    /// Number of execution logs the daemon should include in the analysis.
    /// Clamped to 1–50 by the daemon; the UI allows 1–50.
    @Published var selectedLogCount: Int = 10

    /// The optimization dimension the LLM should concentrate on.
    @Published var optimizationFocus: OptimizationFocus = .general

    // MARK: - Internal State

    /// The task id passed to the last `loadLogs` call; used by `reoptimize`.
    private var currentTaskId: String?

    /// JSON-RPC client injected from the surrounding environment.
    private weak var client: DaemonClient?

    // MARK: - Computed Properties

    /// True when the minimum inputs required to optimize are present.
    var canOptimize: Bool {
        !executionLogs.isEmpty && client != nil
    }

    /// True when at least one execution log has been loaded.
    var hasLogs: Bool {
        !executionLogs.isEmpty
    }

    /// Count of successful runs across all loaded logs.
    var successCount: Int {
        executionLogs.filter { $0.isSuccess }.count
    }

    /// Count of failed runs across all loaded logs.
    var failureCount: Int {
        executionLogs.filter { !$0.isSuccess }.count
    }

    // MARK: - Client Injection

    /// Inject the daemon client.  Called after the view model is created,
    /// mirroring the pattern used by `PromptGeneratorViewModel`.
    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    // MARK: - Log Loading

    /// Fetch the last 10 execution logs for the given task.
    func loadLogs(taskId: String) async {
        guard let client = client else { return }
        currentTaskId = taskId
        isLoadingLogs = true
        error = nil
        do {
            executionLogs = try await client.queryLogs(
                LogQuery(taskId: taskId, status: nil, limit: 10, offset: nil, search: nil)
            )
        } catch {
            // Silently swallow log loading errors — the empty-state UI handles the
            // no-logs case, and connection errors are surfaced by the daemon monitor.
            self.executionLogs = []
        }
        isLoadingLogs = false
    }

    // MARK: - Optimization

    /// Run `prompt.optimize` for the given task using the current settings.
    ///
    /// Uses `selectedLogCount` and `optimizationFocus` to parameterise the RPC
    /// call so the user's panel selections are respected.
    func optimize(taskId: String) async {
        guard let client = client else { return }
        currentTaskId = taskId
        isOptimizing = true
        error = nil
        do {
            let optimized = try await client.optimizePrompt(
                taskId: taskId,
                maxLogs: selectedLogCount,
                focus: optimizationFocus.rawValue
            )
            result = optimized
        } catch {
            self.error = error.localizedDescription
        }
        isOptimizing = false
    }

    /// Re-run optimization with the current `feedbackText`, preserving the
    /// user's `selectedLogCount` and `optimizationFocus` selections.
    func reoptimize() async {
        guard let client = client, let taskId = currentTaskId else { return }
        isOptimizing = true
        error = nil
        let feedback = feedbackText.trimmingCharacters(in: .whitespaces)
        do {
            let optimized = try await client.optimizePrompt(
                taskId: taskId,
                maxLogs: selectedLogCount,
                focus: optimizationFocus.rawValue,
                feedback: feedback.isEmpty ? nil : feedback
            )
            result = optimized
        } catch {
            self.error = error.localizedDescription
        }
        isOptimizing = false
    }

    // MARK: - Apply

    /// Replace the draft command with the optimized command.
    ///
    /// The editor's existing dirty-state comparison automatically detects the
    /// change and activates the unsaved-changes banner.
    func applyOptimization(to draft: inout INTaskDraft) {
        guard let result = result else { return }
        draft.command = result.optimizedCommand
    }

    // MARK: - Reset

    /// Clear result, error, feedback text, and per-run settings.
    ///
    /// Intentionally preserves `executionLogs` so the history summary remains
    /// visible after a discard, and resets `selectedLogCount` and
    /// `optimizationFocus` to their defaults for the next optimization pass.
    func reset() {
        result = nil
        error = nil
        feedbackText = ""
        selectedLogCount = 10
        optimizationFocus = .general
    }
}
