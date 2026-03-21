// MARK: - MockDaemonClient
//
// A test double for DaemonClientProtocol that controls return values and
// records arguments for assertion in unit tests.
//
// NOTE: Once the library/executable split is complete, replace
// DaemonClientProtocol with the real protocol extracted from DaemonClient.swift
// and add `@testable import InternLib` at the top of this file.
//
// Design rationale:
// - DaemonClient is a `final class` with low-level Unix socket I/O in init().
//   Subclassing is impractical for tests that must run without a running daemon.
// - A protocol (DaemonClientProtocol) is the standard Swift approach for
//   seam injection.  The real DaemonClient will conform to it; tests use this mock.
// - Each method returns a pre-configured Result<T, Error> so tests can control
//   success and failure paths without touching the network.

import Foundation

// MARK: - Test Error Helper

enum MockError: LocalizedError {
    case intentional(String)

    var errorDescription: String? {
        switch self {
        case .intentional(let msg): return msg
        }
    }
}

// MARK: - Mock

final class MockDaemonClient: DaemonClientProtocol {

    // MARK: - Configurable return values

    /// Set before calling optimize(taskId:) / reoptimize().
    var optimizePromptResult: Result<PromptOptimizationResult, Error> =
        .success(PromptOptimizationResult.mock())

    /// Set before calling loadLogs(taskId:).
    var queryLogsResult: Result<[ExecutionLog], Error> =
        .success([])

    // MARK: - Recorded arguments

    /// The last `feedback` value passed to optimizePrompt.
    private(set) var lastFeedback: String?

    /// The last task id passed to optimizePrompt.
    private(set) var lastOptimizedTaskId: String?

    /// The last focus string passed to optimizePrompt.
    private(set) var lastFocus: String?

    /// The last maxLogs value passed to optimizePrompt.
    private(set) var lastMaxLogs: Int?

    /// How many times optimizePrompt was called.
    private(set) var optimizeCallCount: Int = 0

    /// How many times queryLogs was called.
    private(set) var queryLogsCallCount: Int = 0

    // MARK: - DaemonClientProtocol conformance

    func optimizePrompt(
        taskId: String,
        maxLogs: Int,
        focus: String,
        feedback: String?
    ) async throws -> PromptOptimizationResult {
        optimizeCallCount += 1
        lastOptimizedTaskId = taskId
        lastMaxLogs = maxLogs
        lastFocus = focus
        lastFeedback = feedback
        return try optimizePromptResult.get()
    }

    func queryLogs(_ query: LogQuery) async throws -> [ExecutionLog] {
        queryLogsCallCount += 1
        return try queryLogsResult.get()
    }
}
