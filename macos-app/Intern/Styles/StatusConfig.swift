import SwiftUI

enum TaskStatusStyle {
    case active
    case paused
    case error
    case success
    case running
    case disabled

    init(from status: TaskStatus) {
        switch status {
        case .active:   self = .active
        case .paused:   self = .paused
        case .error:    self = .error
        case .disabled: self = .disabled
        case .running:  self = .running
        }
    }

    /// Initialize from execution log status string
    init(fromExecStatus status: String) {
        switch status.lowercased() {
        case "success":  self = .success
        case "failed":   self = .error
        case "timeout":  self = .error
        case "killed":   self = .error
        case "skipped":  self = .paused
        default:         self = .disabled
        }
    }

    /// Foreground color
    var color: Color {
        switch self {
        case .active, .success: return .inGreen
        case .paused:           return .inAmber
        case .error:            return .inRed
        case .running:          return .inAccent
        case .disabled:         return .inTextMuted
        }
    }

    /// Badge background
    var background: Color {
        switch self {
        case .active:   return .inGreenBg
        case .success:  return .inGreenBgSubtle
        case .paused:   return .inAmberBg
        case .error:    return .inRedBg
        case .running:  return .inAccentBg
        case .disabled: return Color.inSurfaceRaised
        }
    }

    /// Display label (uppercase in badge)
    var label: String {
        switch self {
        case .active:   return "Active"
        case .paused:   return "Paused"
        case .error:    return "Error"
        case .success:  return "Success"
        case .running:  return "Running"
        case .disabled: return "Disabled"
        }
    }

    /// SF Symbol name
    var sfSymbol: String {
        switch self {
        case .active, .success: return "circle.fill"
        case .paused:           return "pause.fill"
        case .error:            return "xmark"
        case .running:          return "arrow.triangle.2.circlepath"
        case .disabled:         return "minus.circle"
        }
    }
}
