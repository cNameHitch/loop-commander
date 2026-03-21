import Foundation

// MARK: - Agent Entry

/// A single agent entry returned by the registry.
struct AgentEntry: Codable, Identifiable, Hashable {
    var id: String { slug }
    let slug: String
    let name: String
    let description: String
    let category: String
}

// MARK: - Prompt Generate Result

/// Result returned by the `prompt.generate` RPC method.
struct PromptGenerateResult: Codable {
    let promptPath: String
    let taskId: String
    let name: String
    let description: String
    let tags: [String]
    let agents: [String]
    let command: String

    enum CodingKeys: String, CodingKey {
        case promptPath = "prompt_path"
        case taskId = "task_id"
        case name, description, tags, agents, command
    }
}

// MARK: - Registry Refresh Result

/// Result returned by the `registry.refresh` RPC method.
struct RegistryRefreshResult: Codable {
    let agentCount: Int
    let fetchedAt: String

    enum CodingKeys: String, CodingKey {
        case agentCount = "agent_count"
        case fetchedAt = "fetched_at"
    }
}
