import Foundation

/// Result returned by the `prompt.optimize` RPC method.
struct PromptOptimizationResult: Codable {
    let optimizedCommand: String
    let changesSummary: String
    let confidenceScore: Int
    let optimizationCategories: [String]
    let originalCommand: String
    let logsAnalyzed: Int
    let taskId: String

    enum CodingKeys: String, CodingKey {
        case optimizedCommand = "optimized_command"
        case changesSummary = "changes_summary"
        case confidenceScore = "confidence_score"
        case optimizationCategories = "optimization_categories"
        case originalCommand = "original_command"
        case logsAnalyzed = "logs_analyzed"
        case taskId = "task_id"
    }
}

// MARK: - Optimization Focus

/// The aspect of the prompt the optimizer should concentrate on.
///
/// Maps to the `focus` parameter accepted by the `prompt.optimize` RPC method.
enum OptimizationFocus: String, CaseIterable, Identifiable {
    /// Optimize across all dimensions (default).
    case general = "all"
    /// Reduce token usage and execution time.
    case efficiency
    /// Improve output quality and correctness.
    case quality
    /// Make results more reproducible across runs.
    case consistency
    /// Add error handling and retry logic.
    case resilience

    var id: String { rawValue }

    /// Human-readable label shown in the focus picker.
    var displayName: String {
        switch self {
        case .general:     return "All (General)"
        case .efficiency:  return "Efficiency"
        case .quality:     return "Quality"
        case .consistency: return "Consistency"
        case .resilience:  return "Resilience"
        }
    }
}
