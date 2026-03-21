import SwiftUI

struct LogEntryRow: View {
    let log: ExecutionLog
    let isExpanded: Bool
    let onToggle: () -> Void

    private var statusStyle: TaskStatusStyle {
        TaskStatusStyle(fromExecStatus: log.status)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Collapsed summary row
            Button(action: { withAnimation(.inFadeSlide) { onToggle() } }) {
                HStack(spacing: 8) {
                    // Status icon
                    Image(systemName: statusStyle.sfSymbol)
                        .font(.system(size: 10))
                        .foregroundColor(statusStyle.color)
                        .frame(width: 22)

                    // Task name + summary
                    HStack(spacing: 10) {
                        Text(log.taskName)
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextSecondary)
                            .lineLimit(1)
                        let summaryText = log.summary.count > 80
                            ? String(log.summary.prefix(80)) + "\u{2026}"
                            : log.summary
                        Text(summaryText)
                            .font(.inCaption)
                            .foregroundColor(.inTextFaint)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    // Timestamp
                    Text(formatTimestamp(log.startedAt))
                        .font(.inData)
                        .foregroundColor(.inTextSubtle)
                        .frame(width: 140, alignment: .leading)

                    // Duration
                    Text(formatDuration(log.durationSecs))
                        .font(.inData)
                        .foregroundColor(.inTextSubtle)
                        .frame(width: 70, alignment: .leading)

                    // Tokens
                    Text("\(formatTokens(log.tokensUsed)) tok")
                        .font(.inData)
                        .foregroundColor(.inTextSubtle)
                        .frame(width: 80, alignment: .leading)

                    // Cost
                    Text(formatCost(log.costUsd))
                        .font(.inData)
                        .foregroundColor(.inTextSubtle)
                        .frame(width: 70, alignment: .leading)
                }
                .padding(.vertical, 10)
                .padding(.horizontal, 16)
            }
            .buttonStyle(.plain)

            // Expanded detail
            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    Text(log.summary)
                        .font(.inLogSummary)
                        .foregroundColor(.inTextMuted)
                        .lineSpacing(4)
                        .textSelection(.enabled)

                    if !log.output.isEmpty {
                        Text(log.output)
                            .font(.inCode)
                            .foregroundColor(.inTextSecondary)
                            .lineSpacing(5)
                            .padding(14)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Color.inCodeBackground)
                            .overlay(
                                RoundedRectangle(cornerRadius: 6)
                                    .stroke(Color.inBorder, lineWidth: 1)
                            )
                            .cornerRadius(6)
                            .textSelection(.enabled)
                    }
                }
                .padding(.leading, INSpacing.logExpandedInset)
                .padding(.trailing, 16)
                .padding(.bottom, 14)
                .transition(.inFadeSlide)
            }
        }
        .background(isExpanded ? Color.inSurfaceRaised : Color.clear)
        .overlay(alignment: .bottom) {
            Rectangle().fill(Color.inDivider).frame(height: 1)
        }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isButton)
        .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")
        .accessibilityHint("Activate to \(isExpanded ? "collapse" : "expand") log details")
    }
}

// MARK: - Log Table Header

struct LogTableHeader: View {
    var body: some View {
        HStack(spacing: 8) {
            Spacer().frame(width: 22) // status icon column
            Text("Task / Summary")
                .frame(maxWidth: .infinity, alignment: .leading)
            Text("Time")
                .frame(width: 140, alignment: .leading)
            Text("Duration")
                .frame(width: 70, alignment: .leading)
            Text("Tokens")
                .frame(width: 80, alignment: .leading)
            Text("Cost")
                .frame(width: 70, alignment: .leading)
        }
        .font(.inColumnHeader)
        .foregroundColor(.inTextDimmest)
        .textCase(.uppercase)
        .tracking(0.5)
        .padding(.vertical, 10)
        .padding(.horizontal, 16)
        .background(Color.inSurfaceRaised)
        .overlay(alignment: .bottom) {
            Rectangle().fill(Color.inBorder).frame(height: 1)
        }
    }
}
