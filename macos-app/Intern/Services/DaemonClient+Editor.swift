import Foundation

// MARK: - Prompt Editor RPC

extension DaemonClient {

    /// Send the current task draft and user feedback to the daemon for
    /// AI-assisted refinement.
    ///
    /// This call invokes an LLM on the daemon side and may take 30–60 seconds.
    /// The socket read timeout is 5 minutes (set in `connectSync`), so callers
    /// will not observe spurious `DaemonClientError.timeout` errors.
    ///
    /// - Parameters:
    ///   - name: Current task name.
    ///   - command: Current command / prompt text.
    ///   - schedule: Current cron expression.
    ///   - budget: Budget per run in USD.
    ///   - timeout: Timeout in seconds.
    ///   - tags: Current tag list.
    ///   - agents: Current agent slug list.
    ///   - feedback: User's plain-English refinement request (non-empty).
    /// - Returns: A `TaskRefineResult` with the suggested revised fields and
    ///            supporting metadata.
    func refineTask(
        name: String,
        command: String,
        schedule: String,
        budget: Double,
        timeout: Int,
        tags: [String],
        agents: [String],
        feedback: String
    ) async throws -> TaskRefineResult {
        let params: [String: Any] = [
            "name": name,
            "command": command,
            "schedule": schedule,
            "budget": budget,
            "timeout": timeout,
            "tags": tags,
            "agents": agents,
            "feedback": feedback
        ]
        return try await call("prompt.edit", params: params)
    }
}
