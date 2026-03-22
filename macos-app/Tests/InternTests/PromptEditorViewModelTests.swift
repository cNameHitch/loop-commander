// MARK: - SPM Executable Target Limitation
//
// See the comment in PromptOptimizerViewModelTests.swift for full context.
// These tests use local stubs instead of @testable import.

import XCTest

// MARK: - Local Stubs (replace with @testable import InternLib)

// MARK: TaskRefineResult & FieldChange

private struct TaskRefineResult {
    let refinedName: String
    let refinedCommand: String
    let refinedSchedule: String
    let refinedBudget: Double
    let refinedTimeout: Int
    let refinedTags: [String]
    let refinedAgents: [String]
    let changesSummary: String
    let confidenceScore: Int
    let fieldChanges: [String: FieldChange]
    let originalCommand: String
}

private struct FieldChange {
    let `type`: String
    let reason: String
}

// MARK: INTaskDraft

private struct INTaskDraft: Equatable {
    var name: String = ""
    var command: String = ""
    var schedule: String = "*/15 * * * *"
    var maxBudget: Double = 5.0
    var timeoutSecs: Int = 600
    var tags: [String] = []
    var agents: [String] = []
    var workingDir: String = "~/Documents/"
}

// MARK: DaemonEditorClientProtocol
//
// A minimal protocol seam used only by PromptEditorViewModel.  Mirrors the
// refineTask method on the real DaemonClient so the mock can stand in during
// tests without a running daemon or any socket I/O.

private protocol DaemonEditorClientProtocol: AnyObject {
    func refineTask(
        name: String,
        command: String,
        schedule: String,
        budget: Double,
        timeout: Int,
        tags: [String],
        agents: [String],
        feedback: String
    ) async throws -> TaskRefineResult
}

// MARK: PromptEditorViewModel stub
//
// Mirrors the real PromptEditorViewModel.  Once @testable import InternLib is
// available, delete this stub and use the real class.

@MainActor
private class PromptEditorViewModel: ObservableObject {

    // MARK: Published state

    @Published var isEditing: Bool = false
    @Published var result: TaskRefineResult? = nil
    @Published var error: String? = nil
    @Published var feedbackText: String = ""

    // MARK: Internal state

    private weak var client: (AnyObject & DaemonEditorClientProtocol)?

    // MARK: Computed properties

    var canEdit: Bool {
        !feedbackText.trimmingCharacters(in: .whitespaces).isEmpty && client != nil
    }

    // MARK: Client injection

    func setClient(_ client: AnyObject & DaemonEditorClientProtocol) {
        self.client = client
    }

    // MARK: Edit

    func edit(
        name: String,
        command: String,
        schedule: String,
        budget: Double,
        timeout: Int,
        tags: [String],
        agents: [String]
    ) async {
        guard let client = client else { return }
        let feedback = feedbackText.trimmingCharacters(in: .whitespaces)
        guard !feedback.isEmpty else { return }

        isEditing = true
        error = nil
        do {
            result = try await client.refineTask(
                name: name,
                command: command,
                schedule: schedule,
                budget: budget,
                timeout: timeout,
                tags: tags,
                agents: agents,
                feedback: feedback
            )
        } catch {
            self.error = error.localizedDescription
        }
        isEditing = false
    }

    // MARK: Apply

    @discardableResult
    func applyEdit(to draft: inout INTaskDraft) -> INTaskDraft? {
        guard let result = result else { return nil }
        let snapshot = draft
        draft.name = result.refinedName
        draft.command = result.refinedCommand
        draft.schedule = result.refinedSchedule
        draft.maxBudget = result.refinedBudget
        draft.timeoutSecs = result.refinedTimeout
        draft.tags = result.refinedTags
        draft.agents = result.refinedAgents
        return snapshot
    }

    // MARK: Reset

    func reset() {
        result = nil
        error = nil
        feedbackText = ""
    }
}

// MARK: - MockEditorClient

private final class MockEditorClient: DaemonEditorClientProtocol {

    var refineTaskResult: Result<TaskRefineResult, Error> =
        .success(TaskRefineResult.mock())

    private(set) var lastFeedback: String?
    private(set) var lastName: String?
    private(set) var lastCommand: String?
    private(set) var refineCallCount: Int = 0

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
        refineCallCount += 1
        lastName = name
        lastCommand = command
        lastFeedback = feedback
        return try refineTaskResult.get()
    }
}

// MARK: - Factories

extension TaskRefineResult {
    fileprivate static func mock(
        refinedName: String = "Refined Task",
        refinedCommand: String = "claude -p 'refined prompt'",
        refinedSchedule: String = "0 9 * * 1-5",
        refinedBudget: Double = 3.0,
        refinedTimeout: Int = 300,
        refinedTags: [String] = ["auto", "refined"],
        refinedAgents: [String] = [],
        changesSummary: String = "Improved schedule and budget.",
        confidenceScore: Int = 82,
        fieldChanges: [String: FieldChange] = [:],
        originalCommand: String = "claude -p 'original prompt'"
    ) -> TaskRefineResult {
        TaskRefineResult(
            refinedName: refinedName,
            refinedCommand: refinedCommand,
            refinedSchedule: refinedSchedule,
            refinedBudget: refinedBudget,
            refinedTimeout: refinedTimeout,
            refinedTags: refinedTags,
            refinedAgents: refinedAgents,
            changesSummary: changesSummary,
            confidenceScore: confidenceScore,
            fieldChanges: fieldChanges,
            originalCommand: originalCommand
        )
    }
}

// MARK: - Tests

final class PromptEditorViewModelTests: XCTestCase {

    // MARK: - Initial state

    /// A freshly created PromptEditorViewModel must have all-default, inert state.
    func testInitialState() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            XCTAssertFalse(vm.isEditing, "isEditing must start false")
            XCTAssertNil(vm.result, "result must start nil")
            XCTAssertNil(vm.error, "error must start nil")
            XCTAssertEqual(vm.feedbackText, "", "feedbackText must start empty")
            XCTAssertFalse(vm.canEdit, "canEdit must be false — no client and no feedback")
        }
    }

    // MARK: - canEdit

    /// canEdit is false when feedbackText is whitespace only, even with a client set.
    func testCanEditRequiresNonEmptyFeedback() async throws {
        let vm = await PromptEditorViewModel()
        let client = MockEditorClient()

        await MainActor.run {
            vm.setClient(client)
            vm.feedbackText = "   "
            XCTAssertFalse(vm.canEdit,
                           "canEdit must be false when feedbackText is blank whitespace")
        }
    }

    /// canEdit is false when no client is set, even with non-empty feedbackText.
    func testCanEditRequiresClient() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            vm.feedbackText = "make it better"
            XCTAssertFalse(vm.canEdit,
                           "canEdit must be false when no client is injected")
        }
    }

    /// canEdit is true when both a client is set and feedbackText is non-empty.
    func testCanEditTrueWhenClientAndFeedback() async throws {
        let vm = await PromptEditorViewModel()
        let client = MockEditorClient()

        await MainActor.run {
            vm.setClient(client)
            vm.feedbackText = "make it better"
            XCTAssertTrue(vm.canEdit)
        }
    }

    // MARK: - applyEdit

    /// applyEdit(to:) writes all refinable fields from result into the draft,
    /// while leaving workingDir untouched.
    func testApplyEditWritesAllFields() async throws {
        let vm = await PromptEditorViewModel()
        let refined = TaskRefineResult.mock(
            refinedName: "New Name",
            refinedCommand: "claude -p 'new command'",
            refinedSchedule: "0 7 * * *",
            refinedBudget: 4.5,
            refinedTimeout: 450,
            refinedTags: ["tag-a", "tag-b"],
            refinedAgents: ["agent-x"]
        )

        await MainActor.run {
            vm.result = refined
            var draft = INTaskDraft()
            draft.workingDir = "~/Projects/MyApp/"

            vm.applyEdit(to: &draft)

            XCTAssertEqual(draft.name, "New Name")
            XCTAssertEqual(draft.command, "claude -p 'new command'")
            XCTAssertEqual(draft.schedule, "0 7 * * *")
            XCTAssertEqual(draft.maxBudget, 4.5)
            XCTAssertEqual(draft.timeoutSecs, 450)
            XCTAssertEqual(draft.tags, ["tag-a", "tag-b"])
            XCTAssertEqual(draft.agents, ["agent-x"])
            XCTAssertEqual(draft.workingDir, "~/Projects/MyApp/",
                           "applyEdit must not modify workingDir")
        }
    }

    /// applyEdit(to:) returns the pre-apply snapshot for undo purposes.
    func testApplyEditReturnsUndoSnapshot() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            vm.result = TaskRefineResult.mock(
                refinedName: "Post-Edit Name",
                refinedCommand: "claude -p 'post-edit'"
            )

            var draft = INTaskDraft()
            draft.name = "Pre-Edit Name"
            draft.command = "claude -p 'pre-edit'"

            let snapshot = vm.applyEdit(to: &draft)

            XCTAssertNotNil(snapshot, "applyEdit must return a snapshot when result is non-nil")
            XCTAssertEqual(snapshot?.name, "Pre-Edit Name",
                           "snapshot must capture the pre-apply name")
            XCTAssertEqual(snapshot?.command, "claude -p 'pre-edit'",
                           "snapshot must capture the pre-apply command")
            XCTAssertEqual(draft.name, "Post-Edit Name",
                           "draft must reflect the refined name after apply")
        }
    }

    /// applyEdit(to:) is a no-op and returns nil when result is nil.
    func testApplyEditNoopWhenResultNil() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            var draft = INTaskDraft()
            draft.name = "Unchanged Name"
            draft.command = "unchanged command"

            let snapshot = vm.applyEdit(to: &draft)

            XCTAssertNil(snapshot, "applyEdit must return nil when result is nil")
            XCTAssertEqual(draft.name, "Unchanged Name",
                           "draft must be unchanged when result is nil")
            XCTAssertEqual(draft.command, "unchanged command",
                           "draft.command must be unchanged when result is nil")
        }
    }

    // MARK: - reset

    /// reset() clears result, error, and feedbackText.
    func testResetClearsAll() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            vm.result = TaskRefineResult.mock()
            vm.error = "Something broke"
            vm.feedbackText = "please fix the schedule"

            vm.reset()

            XCTAssertNil(vm.result, "reset() must clear result")
            XCTAssertNil(vm.error, "reset() must clear error")
            XCTAssertEqual(vm.feedbackText, "", "reset() must clear feedbackText")
        }
    }

    // MARK: - edit() happy path

    /// edit() stores the returned TaskRefineResult and clears isEditing and error.
    func testEditStoresResult() async throws {
        let vm = await PromptEditorViewModel()
        let client = MockEditorClient()
        let expected = TaskRefineResult.mock(refinedName: "Optimised Task")
        client.refineTaskResult = .success(expected)

        await MainActor.run {
            vm.setClient(client)
            vm.feedbackText = "optimise the schedule"
        }

        await vm.edit(
            name: "Original Task",
            command: "claude -p 'original'",
            schedule: "*/15 * * * *",
            budget: 5.0,
            timeout: 600,
            tags: [],
            agents: []
        )

        await MainActor.run {
            XCTAssertFalse(vm.isEditing)
            XCTAssertNil(vm.error)
            XCTAssertEqual(vm.result?.refinedName, "Optimised Task")
        }
    }

    /// edit() captures and surfaces the error description when the client throws.
    func testEditCapturesError() async throws {
        let vm = await PromptEditorViewModel()
        let client = MockEditorClient()
        client.refineTaskResult = .failure(MockError.intentional("daemon offline"))

        await MainActor.run {
            vm.setClient(client)
            vm.feedbackText = "optimise the schedule"
        }

        await vm.edit(
            name: "Task",
            command: "claude -p 'cmd'",
            schedule: "*/15 * * * *",
            budget: 5.0,
            timeout: 600,
            tags: [],
            agents: []
        )

        await MainActor.run {
            XCTAssertFalse(vm.isEditing)
            XCTAssertNil(vm.result)
            XCTAssertNotNil(vm.error)
        }
    }

    /// edit() is a no-op when feedbackText is blank.
    func testEditNoopWhenFeedbackBlank() async throws {
        let vm = await PromptEditorViewModel()
        let client = MockEditorClient()

        await MainActor.run {
            vm.setClient(client)
            vm.feedbackText = "   "
        }

        await vm.edit(
            name: "Task",
            command: "claude -p 'cmd'",
            schedule: "*/15 * * * *",
            budget: 5.0,
            timeout: 600,
            tags: [],
            agents: []
        )

        await MainActor.run {
            XCTAssertEqual(client.refineCallCount, 0,
                           "edit() must not call refineTask when feedbackText is blank")
            XCTAssertNil(vm.result)
        }
    }

    /// edit() is a no-op when no client has been injected.
    func testEditNoopWhenNoClient() async throws {
        let vm = await PromptEditorViewModel()

        await MainActor.run {
            vm.feedbackText = "make it better"
        }

        await vm.edit(
            name: "Task",
            command: "claude -p 'cmd'",
            schedule: "*/15 * * * *",
            budget: 5.0,
            timeout: 600,
            tags: [],
            agents: []
        )

        await MainActor.run {
            XCTAssertNil(vm.result,
                         "edit() must not mutate result when no client is set")
            XCTAssertNil(vm.error)
        }
    }
}
