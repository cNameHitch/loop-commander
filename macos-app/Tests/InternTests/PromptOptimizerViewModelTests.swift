// MARK: - SPM Executable Target Limitation
//
// Swift Package Manager does not allow a testTarget to depend on an
// executableTarget. Until the Intern executable is split into an
// InternLib library + thin Intern executable, these tests
// cannot use `@testable import Intern`.
//
// To make the split:
//   1. Move all source files except InternApp.swift (the @main entry
//      point) into a new `.target(name: "InternLib", path: "Intern")`
//      library target, excluding the entry point file.
//   2. Create a `.executableTarget(name: "Intern", ...)` whose path
//      contains only InternApp.swift, with a dependency on
//      InternLib.  Any types used from the library must be `public`.
//   3. Change the `.testTarget(name: "InternTests", ...)` dependency
//      from [] to ["InternLib"].
//   4. Replace the stub declarations below with:
//        @testable import InternLib
//      and delete the local stub types.
//
// Until then, run `swift test` from macos-app/ to see the following diagnostic:
//   error: testTarget 'InternTests' cannot depend on
//   executableTarget 'Intern'
//
// The test bodies are complete and will run without modification once the
// import is restored.

import XCTest

// MARK: - Local Stubs (replace with @testable import InternLib)
//
// These stubs mirror the real types and are removed once the library split
// is complete.  They are intentionally minimal — only the interface used by
// PromptOptimizerViewModel is replicated here.

/// Mirror of the real PromptOptimizationResult model.
struct PromptOptimizationResult: Equatable {
    let optimizedCommand: String
    let changesSummary: String
    let confidenceScore: Int
    let optimizationCategories: [String]
    let originalCommand: String
    let logsAnalyzed: Int
    let taskId: String
}

/// Mirror of the real ExecutionLog model.
struct ExecutionLog: Identifiable, Equatable {
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

    var isSuccess: Bool { status == "success" }
}

/// Mirror of OptimizationFocus.
enum OptimizationFocus: String {
    case general = "all"
    case efficiency
    case quality
    case consistency
    case resilience
}

/// Minimal INTaskDraft stub for applyOptimization tests.
struct INTaskDraft {
    var command: String = ""
}

/// Protocol abstraction over DaemonClient — replace with the real
/// DaemonClientProtocol once the library split is complete.
protocol DaemonClientProtocol: AnyObject {
    func optimizePrompt(
        taskId: String,
        maxLogs: Int,
        focus: String,
        feedback: String?
    ) async throws -> PromptOptimizationResult

    func queryLogs(_ query: LogQuery) async throws -> [ExecutionLog]
}

struct LogQuery {
    var taskId: String?
    var status: String?
    var limit: Int?
    var offset: Int?
    var search: String?
}

// MARK: - PromptOptimizerViewModel Stub
//
// This mirrors the real PromptOptimizerViewModel.  Once @testable import
// InternLib is available, delete this stub and use the real class.

@MainActor
class PromptOptimizerViewModel: ObservableObject {
    @Published var isLoadingLogs: Bool = false
    @Published var isOptimizing: Bool = false
    @Published var result: PromptOptimizationResult? = nil
    @Published var error: String? = nil
    @Published var executionLogs: [ExecutionLog] = []
    @Published var feedbackText: String = ""
    @Published var selectedLogCount: Int = 10
    @Published var optimizationFocus: OptimizationFocus = .general

    private var currentTaskId: String?
    private weak var client: (AnyObject & DaemonClientProtocol)?

    var canOptimize: Bool { !executionLogs.isEmpty && client != nil }
    var hasLogs: Bool { !executionLogs.isEmpty }
    var successCount: Int { executionLogs.filter { $0.isSuccess }.count }
    var failureCount: Int { executionLogs.filter { !$0.isSuccess }.count }

    func setClient(_ client: AnyObject & DaemonClientProtocol) {
        self.client = client
    }

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
            executionLogs = []
        }
        isLoadingLogs = false
    }

    func optimize(taskId: String) async {
        guard let client = client else { return }
        currentTaskId = taskId
        isOptimizing = true
        error = nil
        do {
            result = try await client.optimizePrompt(
                taskId: taskId,
                maxLogs: selectedLogCount,
                focus: optimizationFocus.rawValue,
                feedback: nil
            )
        } catch {
            self.error = error.localizedDescription
        }
        isOptimizing = false
    }

    func reoptimize() async {
        guard let client = client, let taskId = currentTaskId else { return }
        isOptimizing = true
        error = nil
        let feedback = feedbackText.trimmingCharacters(in: .whitespaces)
        do {
            result = try await client.optimizePrompt(
                taskId: taskId,
                maxLogs: selectedLogCount,
                focus: optimizationFocus.rawValue,
                feedback: feedback.isEmpty ? nil : feedback
            )
        } catch {
            self.error = error.localizedDescription
        }
        isOptimizing = false
    }

    func applyOptimization(to draft: inout INTaskDraft) {
        guard let result = result else { return }
        draft.command = result.optimizedCommand
    }

    func reset() {
        result = nil
        error = nil
        feedbackText = ""
        selectedLogCount = 10
        optimizationFocus = .general
    }
}

// MARK: - Tests

final class PromptOptimizerViewModelTests: XCTestCase {

    // MARK: - Initial State

    /// A freshly created view model must have the correct default state:
    /// no result, no error, no logs, not optimizing, canOptimize == false.
    func testInitialState() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            XCTAssertFalse(vm.isOptimizing)
            XCTAssertFalse(vm.isLoadingLogs)
            XCTAssertNil(vm.result)
            XCTAssertNil(vm.error)
            XCTAssertTrue(vm.executionLogs.isEmpty)
            XCTAssertFalse(vm.canOptimize, "canOptimize must be false — no client and no logs")
            XCTAssertFalse(vm.hasLogs)
            XCTAssertEqual(vm.feedbackText, "")
            XCTAssertEqual(vm.selectedLogCount, 10)
            XCTAssertEqual(vm.optimizationFocus, .general)
        }
    }

    // MARK: - canOptimize

    /// canOptimize is false when there is no client, even if logs are present.
    func testCanOptimizeRequiresClient() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            vm.executionLogs = [ExecutionLog.mock()]
            XCTAssertFalse(vm.canOptimize, "canOptimize must be false — no client")
        }
    }

    /// canOptimize is false when there is a client but no logs.
    func testCanOptimizeRequiresLogs() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()

        await MainActor.run {
            vm.setClient(client)
            XCTAssertFalse(vm.canOptimize, "canOptimize must be false — no logs")
        }
    }

    /// canOptimize is true when both a client and logs are present.
    func testCanOptimizeTrueWithClientAndLogs() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()

        await MainActor.run {
            vm.setClient(client)
            vm.executionLogs = [ExecutionLog.mock()]
            XCTAssertTrue(vm.canOptimize)
        }
    }

    // MARK: - hasLogs / successCount / failureCount

    func testHasLogsReflectsExecutionLogsCount() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            XCTAssertFalse(vm.hasLogs)
            vm.executionLogs = [ExecutionLog.mock(status: "success")]
            XCTAssertTrue(vm.hasLogs)
        }
    }

    func testSuccessAndFailureCounts() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            vm.executionLogs = [
                ExecutionLog.mock(id: 1, status: "success"),
                ExecutionLog.mock(id: 2, status: "success"),
                ExecutionLog.mock(id: 3, status: "failed"),
            ]
            XCTAssertEqual(vm.successCount, 2)
            XCTAssertEqual(vm.failureCount, 1)
        }
    }

    // MARK: - reset()

    /// reset() clears result, error, feedbackText, selectedLogCount, and
    /// optimizationFocus, but intentionally preserves executionLogs.
    func testResetPreservesLogs() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            // Set non-default state
            vm.result = PromptOptimizationResult.mock()
            vm.error = "Something went wrong"
            vm.feedbackText = "Use fewer tokens"
            vm.selectedLogCount = 25
            vm.optimizationFocus = .efficiency
            vm.executionLogs = [ExecutionLog.mock()]

            vm.reset()

            XCTAssertNil(vm.result, "reset() must clear result")
            XCTAssertNil(vm.error, "reset() must clear error")
            XCTAssertEqual(vm.feedbackText, "", "reset() must clear feedbackText")
            XCTAssertEqual(vm.selectedLogCount, 10, "reset() must restore selectedLogCount to 10")
            XCTAssertEqual(vm.optimizationFocus, .general, "reset() must restore optimizationFocus to .general")
            XCTAssertFalse(vm.executionLogs.isEmpty, "reset() must preserve executionLogs")
        }
    }

    // MARK: - applyOptimization()

    /// applyOptimization() writes the optimized command into the draft.
    func testApplyOptimizationWritesCommand() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            let optimized = PromptOptimizationResult.mock(optimizedCommand: "claude -p 'improved prompt'")
            vm.result = optimized
            var draft = INTaskDraft()
            draft.command = "old command"

            vm.applyOptimization(to: &draft)

            XCTAssertEqual(draft.command, "claude -p 'improved prompt'")
        }
    }

    /// applyOptimization() is a no-op when result is nil.
    func testApplyOptimizationNoopWhenNoResult() async throws {
        let vm = await PromptOptimizerViewModel()

        await MainActor.run {
            var draft = INTaskDraft()
            draft.command = "original command"
            vm.applyOptimization(to: &draft)
            XCTAssertEqual(draft.command, "original command")
        }
    }

    // MARK: - optimize() happy path

    /// optimize() sets isOptimizing during the call and clears it after,
    /// and stores the returned result.
    func testOptimizeStoresResult() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()
        let expected = PromptOptimizationResult.mock(optimizedCommand: "optimized!")

        await MainActor.run {
            vm.setClient(client)
            vm.executionLogs = [ExecutionLog.mock()]
            client.optimizePromptResult = .success(expected)
        }

        await vm.optimize(taskId: "task-1")

        await MainActor.run {
            XCTAssertFalse(vm.isOptimizing)
            XCTAssertEqual(vm.result, expected)
            XCTAssertNil(vm.error)
        }
    }

    /// optimize() captures the error description when the client throws.
    func testOptimizeCapturesError() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()

        await MainActor.run {
            vm.setClient(client)
            vm.executionLogs = [ExecutionLog.mock()]
            client.optimizePromptResult = .failure(MockError.intentional("daemon unavailable"))
        }

        await vm.optimize(taskId: "task-1")

        await MainActor.run {
            XCTAssertFalse(vm.isOptimizing)
            XCTAssertNil(vm.result)
            XCTAssertNotNil(vm.error)
        }
    }

    // MARK: - reoptimize()

    /// reoptimize() passes the feedback text to the client.
    func testReoptimizePassesFeedback() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()
        let expected = PromptOptimizationResult.mock()

        await MainActor.run {
            vm.setClient(client)
            vm.executionLogs = [ExecutionLog.mock()]
            client.optimizePromptResult = .success(expected)
            vm.feedbackText = "  be more concise  "
        }

        // First optimize to set currentTaskId
        await vm.optimize(taskId: "task-2")
        // Then reoptimize with feedback
        await vm.reoptimize()

        await MainActor.run {
            XCTAssertEqual(client.lastFeedback, "be more concise",
                           "reoptimize() must trim whitespace from feedbackText")
            XCTAssertFalse(vm.isOptimizing)
        }
    }

    /// reoptimize() sends nil feedback when feedbackText is blank.
    func testReoptimizeNilFeedbackWhenBlank() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()
        client.optimizePromptResult = .success(PromptOptimizationResult.mock())

        await MainActor.run {
            vm.setClient(client)
            vm.executionLogs = [ExecutionLog.mock()]
            vm.feedbackText = "   "
        }

        await vm.optimize(taskId: "task-3")
        await vm.reoptimize()

        await MainActor.run {
            XCTAssertNil(client.lastFeedback,
                         "blank feedbackText must become nil in the RPC call")
        }
    }

    // MARK: - loadLogs()

    /// loadLogs() populates executionLogs on success.
    func testLoadLogsPopulatesLogs() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()
        let mockLogs = [ExecutionLog.mock(id: 1), ExecutionLog.mock(id: 2)]

        await MainActor.run {
            vm.setClient(client)
            client.queryLogsResult = .success(mockLogs)
        }

        await vm.loadLogs(taskId: "task-4")

        await MainActor.run {
            XCTAssertEqual(vm.executionLogs, mockLogs)
            XCTAssertFalse(vm.isLoadingLogs)
        }
    }

    /// loadLogs() silently leaves executionLogs empty on failure.
    func testLoadLogsSwallowsError() async throws {
        let vm = await PromptOptimizerViewModel()
        let client = MockDaemonClient()

        await MainActor.run {
            vm.setClient(client)
            client.queryLogsResult = .failure(MockError.intentional("network error"))
        }

        await vm.loadLogs(taskId: "task-5")

        await MainActor.run {
            XCTAssertTrue(vm.executionLogs.isEmpty,
                          "loadLogs() must leave executionLogs empty on error")
            XCTAssertFalse(vm.isLoadingLogs)
        }
    }
}
