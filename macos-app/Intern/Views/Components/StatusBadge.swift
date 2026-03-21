import SwiftUI

struct StatusBadge: View {
    let status: TaskStatus

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var style: TaskStatusStyle {
        TaskStatusStyle(from: status)
    }

    var body: some View {
        HStack(spacing: 5) {
            if status == .running {
                Image(systemName: style.sfSymbol)
                    .font(.system(size: 8, weight: .bold))
                    .modifier(ConditionalRotation(animate: !reduceMotion))
            } else {
                Image(systemName: style.sfSymbol)
                    .font(.system(size: 8, weight: .bold))
            }
            Text(style.label)
                .font(.inBadge)
                .textCase(.uppercase)
                .tracking(0.5)
        }
        .foregroundColor(style.color)
        .padding(.horizontal, 10)
        .padding(.vertical, 3)
        .background(style.background)
        .cornerRadius(INRadius.badge)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Status: \(style.label)")
    }
}

/// StatusBadge for execution log status (string-based)
struct ExecStatusBadge: View {
    let status: String

    private var style: TaskStatusStyle {
        TaskStatusStyle(fromExecStatus: status)
    }

    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: style.sfSymbol)
                .font(.system(size: 8, weight: .bold))
            Text(status.capitalized)
                .font(.inBadge)
                .textCase(.uppercase)
                .tracking(0.5)
        }
        .foregroundColor(style.color)
        .padding(.horizontal, 10)
        .padding(.vertical, 3)
        .background(style.background)
        .cornerRadius(INRadius.badge)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Status: \(status)")
    }
}
