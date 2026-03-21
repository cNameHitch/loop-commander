import Foundation

/// ViewModel for the AI prompt generator panel.
///
/// Manages agent selection, intent input, and the lifecycle of a `prompt.generate`
/// RPC call.  Follows the same `@MainActor` / `ObservableObject` / weak-client
/// patterns used throughout the app.
@MainActor
class PromptGeneratorViewModel: ObservableObject {

    // MARK: - Published State

    /// The natural-language intent the user wants to turn into a task prompt.
    @Published var intent: String = ""

    /// The set of agent slugs the user has selected to include in generation.
    @Published var selectedAgents: Set<String> = []

    /// Optional free-text feedback used when calling `regenerate(workingDir:)`.
    @Published var feedbackText: String = ""

    /// The full list of agents returned by the registry.
    @Published var agents: [AgentEntry] = []

    /// True while the `registry.list` RPC call is in flight.
    @Published var isLoadingAgents: Bool = false

    /// True while the `prompt.generate` RPC call is in flight.
    @Published var isGenerating: Bool = false

    /// The most recent generation result; nil until the first successful generate.
    @Published var result: PromptGenerateResult? = nil

    /// Non-nil when an error should be surfaced to the user.
    @Published var error: String? = nil

    /// Incremented on each successful generate or regenerate call.
    @Published var generationCount: Int = 0

    // MARK: - Computed Properties

    /// True when the minimum inputs required to generate are present.
    var canGenerate: Bool {
        !intent.trimmingCharacters(in: .whitespaces).isEmpty && !selectedAgents.isEmpty
    }

    /// Ordered, deduplicated list of category names across all loaded agents.
    var agentCategories: [String] {
        var seen = Set<String>()
        return agents.compactMap { agent in
            guard !seen.contains(agent.category) else { return nil }
            seen.insert(agent.category)
            return agent.category
        }
    }

    // MARK: - Client

    private weak var client: DaemonClient?

    /// Inject the daemon client.  Called after the view model is created, mirroring
    /// the pattern used by `TaskEditorViewModel` and `EditorViewModel`.
    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    // MARK: - Agent Loading

    /// Fetch the full agent list from the registry.
    func loadAgents() async {
        guard let client = client else { return }
        isLoadingAgents = true
        error = nil
        do {
            agents = try await client.listAgents()
        } catch {
            self.error = error.localizedDescription
        }
        isLoadingAgents = false
    }

    /// Force-refresh the agent registry and then reload.
    func refreshRegistry() async {
        guard let client = client else { return }
        isLoadingAgents = true
        error = nil
        do {
            _ = try await client.refreshAgentRegistry()
            agents = try await client.listAgents()
        } catch {
            self.error = error.localizedDescription
        }
        isLoadingAgents = false
    }

    // MARK: - Generation

    /// Generate a prompt from the current `intent` and `selectedAgents`.
    ///
    /// - Parameter workingDir: The working directory to pass to the daemon.
    func generate(workingDir: String) async {
        guard let client = client, canGenerate else { return }
        isGenerating = true
        error = nil
        do {
            let generated = try await client.generatePrompt(
                intent: intent.trimmingCharacters(in: .whitespaces),
                agents: Array(selectedAgents),
                workingDir: workingDir
            )
            result = generated
            generationCount += 1
        } catch {
            self.error = error.localizedDescription
        }
        isGenerating = false
    }

    /// Re-run generation, appending `feedbackText` to the original intent so the
    /// daemon can take the previous result and the user's notes into account.
    ///
    /// - Parameter workingDir: The working directory to pass to the daemon.
    func regenerate(workingDir: String) async {
        guard !feedbackText.trimmingCharacters(in: .whitespaces).isEmpty else {
            await generate(workingDir: workingDir)
            return
        }
        let combined = intent.trimmingCharacters(in: .whitespaces)
            + "\n\nFeedback: "
            + feedbackText.trimmingCharacters(in: .whitespaces)
        let originalIntent = intent
        intent = combined
        await generate(workingDir: workingDir)
        // Restore the original intent so the field does not show the combined string.
        intent = originalIntent
    }

    // MARK: - Agent Selection

    /// Toggle membership of an agent slug in `selectedAgents`.
    func toggleAgent(_ slug: String) {
        if selectedAgents.contains(slug) {
            selectedAgents.remove(slug)
        } else {
            selectedAgents.insert(slug)
        }
    }

    /// Return the `AgentEntry` for the given slug, or nil if not loaded yet.
    func agentEntry(for slug: String) -> AgentEntry? {
        agents.first { $0.slug == slug }
    }
}
