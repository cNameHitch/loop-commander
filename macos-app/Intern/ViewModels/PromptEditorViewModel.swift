import Foundation

/// ViewModel for the AI prompt editor panel.
///
/// Manages the lifecycle of a `prompt.edit` RPC call and the undo state for
/// session-scoped reverting.  Follows the same `@MainActor` / `ObservableObject`
/// / weak-client pattern used by `PromptOptimizerViewModel`.
@MainActor
class PromptEditorViewModel: ObservableObject {

    // MARK: - Published State

    /// True while the `prompt.edit` RPC call is in flight.
    @Published var isEditing: Bool = false

    /// The most recent edit result; nil until the first successful call.
    @Published var result: TaskRefineResult? = nil

    /// Non-nil when an error should be surfaced to the user.
    @Published var error: String? = nil

    /// The user's feedback text entered in the panel.
    @Published var feedbackText: String = ""

    // MARK: - Internal State

    /// JSON-RPC client injected from the surrounding environment.
    private weak var client: DaemonClient?

    // MARK: - Computed Properties

    /// True when the minimum inputs required to edit are present.
    var canEdit: Bool {
        !feedbackText.trimmingCharacters(in: .whitespaces).isEmpty && client != nil
    }

    // MARK: - Client Injection

    /// Inject the daemon client.  Called after the view model is created,
    /// mirroring the pattern used by `PromptOptimizerViewModel`.
    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    // MARK: - Edit

    /// Run `prompt.edit` using the current draft fields and `feedbackText`.
    ///
    /// The caller passes the full draft snapshot so the VM does not need to
    /// hold a reference to the editor draft itself.
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

    // MARK: - Apply

    /// Apply the refined fields to the given draft.
    ///
    /// Updates all refinable fields: name, command, schedule, budget, timeout,
    /// tags, and agents.  The `working_dir` field is intentionally not touched
    /// (SSD §6.2).
    ///
    /// - Returns: The pre-apply snapshot for undo purposes, or nil if no result.
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

    // MARK: - Reset

    /// Clear result, error, and feedback text.
    func reset() {
        result = nil
        error = nil
        feedbackText = ""
    }
}
