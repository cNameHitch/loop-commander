// MARK: - Test Helpers
//
// Convenience factory methods for building test fixtures.
//
// NOTE: Once @testable import LoopCommanderLib is available, remove the local
// stub types from PromptOptimizerViewModelTests.swift and use the real model
// types here.  The factory method signatures remain identical.

import Foundation

// MARK: - PromptOptimizationResult factories

extension PromptOptimizationResult {

    /// Returns a fully-populated mock result.
    ///
    /// Every parameter has a sensible default so individual tests only need to
    /// override the fields they care about.
    static func mock(
        optimizedCommand: String = "claude -p 'optimized prompt'",
        changesSummary: String = "Reduced verbosity and added error handling",
        confidenceScore: Int = 75,
        optimizationCategories: [String] = ["efficiency", "resilience"],
        originalCommand: String = "claude -p 'original prompt'",
        logsAnalyzed: Int = 10,
        taskId: String = "test-task-id"
    ) -> PromptOptimizationResult {
        PromptOptimizationResult(
            optimizedCommand: optimizedCommand,
            changesSummary: changesSummary,
            confidenceScore: confidenceScore,
            optimizationCategories: optimizationCategories,
            originalCommand: originalCommand,
            logsAnalyzed: logsAnalyzed,
            taskId: taskId
        )
    }
}

// MARK: - ExecutionLog factories

extension ExecutionLog {

    /// Returns a fully-populated mock log entry.
    ///
    /// Defaults to a successful 30-second run. Override `status` with "failed"
    /// or "timeout" to simulate non-success outcomes.
    static func mock(
        id: Int = 1,
        taskId: String = "test-task-id",
        taskName: String = "Test Task",
        startedAt: String = "2026-03-21T10:00:00Z",
        finishedAt: String = "2026-03-21T10:00:30Z",
        durationSecs: Int = 30,
        exitCode: Int = 0,
        status: String = "success",
        stdout: String = "Task completed successfully.",
        stderr: String = "",
        tokensUsed: Int? = 1_500,
        costUsd: Double? = 0.03,
        summary: String = "Completed in 30 seconds."
    ) -> ExecutionLog {
        ExecutionLog(
            id: id,
            taskId: taskId,
            taskName: taskName,
            startedAt: startedAt,
            finishedAt: finishedAt,
            durationSecs: durationSecs,
            exitCode: exitCode,
            status: status,
            stdout: stdout,
            stderr: stderr,
            tokensUsed: tokensUsed,
            costUsd: costUsd,
            summary: summary
        )
    }

    /// Returns a mock failed log entry.
    static func mockFailed(
        id: Int = 99,
        taskId: String = "test-task-id",
        exitCode: Int = 1,
        summary: String = "Process exited with code 1."
    ) -> ExecutionLog {
        mock(
            id: id,
            taskId: taskId,
            exitCode: exitCode,
            status: "failed",
            stdout: "",
            stderr: "Error: unexpected failure",
            summary: summary
        )
    }
}
