import Foundation

// MARK: - Prompt Optimizer RPC

extension DaemonClient {

    /// Analyze execution history for a task and return an optimized command.
    ///
    /// This call invokes an LLM on the daemon side and may take 30–60 seconds.
    /// The socket read timeout is 30 seconds by default, so callers should be
    /// aware that this method may surface `DaemonClientError.timeout` on slow
    /// networks or heavily loaded daemons.
    ///
    /// - Parameters:
    ///   - taskId: The id of the task to optimize.
    ///   - maxLogs: Maximum number of execution logs to include in analysis
    ///              (default 10, clamped to 1–50 by the daemon).
    ///   - focus: Optimization focus area — one of "all", "efficiency",
    ///            "quality", "consistency", or "resilience".  Default "all".
    ///   - feedback: Optional free-text notes from the user fed into a
    ///               re-optimization pass.
    /// - Returns: A `PromptOptimizationResult` with the suggested command and
    ///            supporting metadata.
    func optimizePrompt(
        taskId: String,
        maxLogs: Int = 10,
        focus: String = "all",
        feedback: String? = nil
    ) async throws -> PromptOptimizationResult {
        var params: [String: Any] = [
            "task_id": taskId,
            "max_logs": maxLogs,
            "focus": focus
        ]
        if let feedback = feedback, !feedback.isEmpty {
            params["feedback"] = feedback
        }
        return try await call("prompt.optimize", params: params)
    }
}
