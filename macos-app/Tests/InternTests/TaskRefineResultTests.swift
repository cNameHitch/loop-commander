// MARK: - SPM Executable Target Limitation
//
// See the comment in PromptOptimizerViewModelTests.swift for full context.
// These tests use local stubs instead of @testable import.

import XCTest

// MARK: - Local Stubs (replace with @testable import InternLib)
//
// These stubs mirror the Codable contract of the real types.  CodingKeys must
// stay in sync with TaskRefineResult.swift and FieldChange in the same file.

private struct TaskRefineResult: Codable {
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

private struct FieldChange: Codable {
    let `type`: String
    let reason: String

    enum CodingKeys: String, CodingKey {
        case `type` = "type"
        case reason
    }
}

// MARK: - Tests

final class TaskRefineResultTests: XCTestCase {

    // MARK: - Full decode

    /// Decoding a complete JSON payload with snake_case keys populates every field.
    func testDecodesFromSnakeCaseJSON() throws {
        let json = """
        {
            "refined_name": "Daily Digest",
            "refined_command": "claude -p 'summarise news'",
            "refined_schedule": "0 8 * * 1-5",
            "refined_budget": 2.5,
            "refined_timeout": 300,
            "refined_tags": ["news", "digest"],
            "refined_agents": ["agent-1"],
            "changes_summary": "Switched to weekdays only and capped budget.",
            "confidence_score": 88,
            "field_changes": {
                "schedule": {"type": "expression_changed", "reason": "Changed to weekdays."},
                "budget": {"type": "value_decreased", "reason": "Reduced to save cost."}
            },
            "original_command": "claude -p 'get news'"
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let result = try JSONDecoder().decode(TaskRefineResult.self, from: data)

        XCTAssertEqual(result.refinedName, "Daily Digest")
        XCTAssertEqual(result.refinedCommand, "claude -p 'summarise news'")
        XCTAssertEqual(result.refinedSchedule, "0 8 * * 1-5")
        XCTAssertEqual(result.refinedBudget, 2.5)
        XCTAssertEqual(result.refinedTimeout, 300)
        XCTAssertEqual(result.refinedTags, ["news", "digest"])
        XCTAssertEqual(result.refinedAgents, ["agent-1"])
        XCTAssertEqual(result.changesSummary, "Switched to weekdays only and capped budget.")
        XCTAssertEqual(result.confidenceScore, 88)
        XCTAssertEqual(result.originalCommand, "claude -p 'get news'")
        XCTAssertEqual(result.fieldChanges.count, 2)
    }

    // MARK: - FieldChange decode

    /// Decoding a standalone FieldChange captures both `type` and `reason`.
    func testFieldChangeDecodesTypeAndReason() throws {
        let json = """
        {"type": "expression_changed", "reason": "Changed to weekdays."}
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let change = try JSONDecoder().decode(FieldChange.self, from: data)

        XCTAssertEqual(change.type, "expression_changed")
        XCTAssertEqual(change.reason, "Changed to weekdays.")
    }

    // MARK: - Round-trip

    /// Encoding then decoding a TaskRefineResult preserves all field values.
    func testRoundTrip() throws {
        let original = TaskRefineResult(
            refinedName: "Round Trip Task",
            refinedCommand: "claude -p 'do something'",
            refinedSchedule: "*/30 * * * *",
            refinedBudget: 1.0,
            refinedTimeout: 120,
            refinedTags: ["rt", "test"],
            refinedAgents: [],
            changesSummary: "No changes needed.",
            confidenceScore: 95,
            fieldChanges: ["name": FieldChange(type: "no_change", reason: "Already optimal.")],
            originalCommand: "claude -p 'do something'"
        )

        let encoded = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(TaskRefineResult.self, from: encoded)

        XCTAssertEqual(decoded.refinedName, original.refinedName)
        XCTAssertEqual(decoded.refinedCommand, original.refinedCommand)
        XCTAssertEqual(decoded.refinedSchedule, original.refinedSchedule)
        XCTAssertEqual(decoded.refinedBudget, original.refinedBudget)
        XCTAssertEqual(decoded.refinedTimeout, original.refinedTimeout)
        XCTAssertEqual(decoded.refinedTags, original.refinedTags)
        XCTAssertEqual(decoded.refinedAgents, original.refinedAgents)
        XCTAssertEqual(decoded.changesSummary, original.changesSummary)
        XCTAssertEqual(decoded.confidenceScore, original.confidenceScore)
        XCTAssertEqual(decoded.originalCommand, original.originalCommand)
        XCTAssertEqual(decoded.fieldChanges.count, original.fieldChanges.count)
        XCTAssertEqual(decoded.fieldChanges["name"]?.type, "no_change")
        XCTAssertEqual(decoded.fieldChanges["name"]?.reason, "Already optimal.")
    }

    // MARK: - Empty field_changes

    /// A JSON payload with an empty `field_changes` object decodes to an empty dictionary.
    func testDecodesEmptyFieldChanges() throws {
        let json = """
        {
            "refined_name": "Test",
            "refined_command": "claude -p 'x'",
            "refined_schedule": "*/15 * * * *",
            "refined_budget": 5.0,
            "refined_timeout": 600,
            "refined_tags": [],
            "refined_agents": [],
            "changes_summary": "Nothing changed.",
            "confidence_score": 100,
            "field_changes": {},
            "original_command": "claude -p 'x'"
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let result = try JSONDecoder().decode(TaskRefineResult.self, from: data)

        XCTAssertTrue(result.fieldChanges.isEmpty,
                      "field_changes must decode to an empty dictionary when the JSON object is empty")
    }

    // MARK: - Missing required field

    /// A JSON payload missing the required `refined_command` key must throw a decoding error.
    func testDecodeFailsWithMissingRequiredField() throws {
        let json = """
        {
            "refined_name": "Incomplete",
            "refined_schedule": "*/15 * * * *",
            "refined_budget": 5.0,
            "refined_timeout": 600,
            "refined_tags": [],
            "refined_agents": [],
            "changes_summary": "Missing command.",
            "confidence_score": 50,
            "field_changes": {},
            "original_command": "claude -p 'x'"
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))

        XCTAssertThrowsError(
            try JSONDecoder().decode(TaskRefineResult.self, from: data),
            "Decoding must throw when refined_command is absent"
        )
    }
}
