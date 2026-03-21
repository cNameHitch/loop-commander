import SwiftUI

struct TaskRow: View {
    let task: INTask
    let isSelected: Bool

    private var successRate: Int {
        guard task.runCount > 0 else { return 0 }
        return Int(round(Double(task.successCount) / Double(task.runCount) * 100))
    }

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 0) {
            // Column 1: Task name + working dir
            VStack(alignment: .leading, spacing: 3) {
                Text(task.name)
                    .font(.inBodyBold)
                    .foregroundColor(.inTextPrimary)
                    .lineLimit(1)
                Text(abbreviatePath(task.workingDir))
                    .font(.inDataSmall)
                    .foregroundColor(.inTextSubtle)
                    .lineLimit(1)
            }
            .frame(minWidth: 120, maxWidth: .infinity, alignment: .leading)

            // Column 2: Schedule
            Text(task.scheduleHuman)
                .font(.inDataMedium)
                .foregroundColor(.inTextMuted)
                .lineLimit(1)
                .frame(minWidth: 80, maxWidth: 160, alignment: .leading)

            // Column 3: Status
            StatusBadge(status: task.status)
                .frame(minWidth: 80, maxWidth: 120, alignment: .leading)

            // Column 4: Runs
            Text("\(task.runCount)")
                .font(.inDataMedium)
                .foregroundColor(.inTextSubtle)
                .frame(width: 50, alignment: .leading)

            // Column 5: Health
            Text(task.runCount > 0 ? "\(successRate)%" : "\u{2014}")
                .font(.inDataMedium)
                .foregroundColor(task.runCount > 0 ? .inHealthColor(for: successRate) : .inTextSubtle)
                .frame(width: 50, alignment: .leading)
        }
        .padding(.vertical, 14)
        .padding(.horizontal, 20)
        .background(
            isSelected
                ? Color.inAccentBgSubtle
                : (isHovered ? Color.inSurfaceRaised : Color.clear)
        )
        .overlay(alignment: .leading) {
            if isSelected {
                Rectangle()
                    .fill(Color.inSelectedBorder)
                    .frame(width: 2)
            }
        }
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.inDivider)
                .frame(height: 1)
        }
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovered = hovering
        }
        .animation(.inQuick, value: isSelected)
        .animation(.inQuick, value: isHovered)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(task.name), \(task.status.rawValue), \(task.scheduleHuman)")
        .accessibilityValue("\(successRate)% success rate, \(task.runCount) runs")
        .accessibilityAddTraits(.isButton)
    }
}

// MARK: - Task Table Header

struct TaskTableHeader: View {
    var body: some View {
        HStack(spacing: 0) {
            Text("Task")
                .frame(minWidth: 120, maxWidth: .infinity, alignment: .leading)
            Text("Schedule")
                .frame(minWidth: 80, maxWidth: 160, alignment: .leading)
            Text("Status")
                .frame(minWidth: 80, maxWidth: 120, alignment: .leading)
            Text("Runs")
                .frame(width: 50, alignment: .leading)
            Text("Health")
                .frame(width: 50, alignment: .leading)
        }
        .lineLimit(1)
        .font(.inColumnHeader)
        .foregroundColor(.inTextFaint)
        .textCase(.uppercase)
        .tracking(0.5)
        .padding(.vertical, 10)
        .padding(.horizontal, 20)
        .background(Color.inSurfaceRaised)
        .overlay(alignment: .bottom) {
            Rectangle().fill(Color.inBorder).frame(height: 1)
        }
    }
}
