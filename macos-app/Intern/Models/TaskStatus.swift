import Foundation

/// Task lifecycle status, mirroring Rust `TaskStatus` enum.
enum TaskStatus: String, Codable, Hashable, CaseIterable {
    case active
    case paused
    case error
    case disabled
    case running

    var displayName: String {
        rawValue.capitalized
    }
}
