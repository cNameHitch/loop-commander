import Foundation

// MARK: - Task Refine Result

/// Result returned by the `prompt.edit` RPC method.
struct TaskRefineResult: Codable {
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

    enum CodingKeys: String, CodingKey {
        case refinedName = "refined_name"
        case refinedCommand = "refined_command"
        case refinedSchedule = "refined_schedule"
        case refinedBudget = "refined_budget"
        case refinedTimeout = "refined_timeout"
        case refinedTags = "refined_tags"
        case refinedAgents = "refined_agents"
        case changesSummary = "changes_summary"
        case confidenceScore = "confidence_score"
        case fieldChanges = "field_changes"
        case originalCommand = "original_command"
    }
}

// MARK: - Field Change

/// A single field-level change descriptor in a `TaskRefineResult`.
struct FieldChange: Codable {
    let `type`: String
    let reason: String

    enum CodingKeys: String, CodingKey {
        case `type` = "type"
        case reason
    }
}
