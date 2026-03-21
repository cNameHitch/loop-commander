import SwiftUI

enum SidebarItem: Hashable {
    case tasks
    case editor
    case logs
}

struct SidebarView: View {
    @Binding var selection: SidebarItem?
    let activeCount: Int
    let editorIsDirty: Bool

    var body: some View {
        VStack(spacing: 0) {
            // App branding header
            HStack(spacing: 14) {
                RoundedRectangle(cornerRadius: 8)
                    .fill(
                        LinearGradient(
                            colors: [.inAccent, .inAccentDeep],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .frame(width: 32, height: 32)
                    .overlay(
                        Image(systemName: "person.fill")
                            .font(.system(size: 15, weight: .heavy))
                            .foregroundColor(.white)
                    )
                VStack(alignment: .leading, spacing: 2) {
                    Text("Intern")
                        .font(.inTitle)
                        .foregroundColor(.inTextPrimary)
                    Text("CLAUDE CODE \u{00B7} LAUNCHD \u{00B7} \(activeCount) RUNNING")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundColor(.inTextFaint)
                        .tracking(0.5)
                }
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.top, 16)
            .padding(.bottom, 12)

            // Navigation buttons
            VStack(spacing: 2) {
                sidebarButton(
                    title: "Tasks",
                    icon: "list.bullet.rectangle",
                    item: .tasks,
                    badge: activeCount > 0 ? "\(activeCount)" : nil,
                    dirtyDot: false
                )
                sidebarButton(
                    title: "Editor",
                    icon: "pencil.and.outline",
                    item: .editor,
                    badge: nil,
                    dirtyDot: editorIsDirty
                )
                sidebarButton(
                    title: "Logs",
                    icon: "doc.text.magnifyingglass",
                    item: .logs,
                    badge: nil,
                    dirtyDot: false
                )
            }
            .padding(.horizontal, 8)

            Spacer()
        }
        .background(Color.inSurface)
        .overlay(alignment: .trailing) {
            Rectangle()
                .fill(Color.inSeparator)
                .frame(width: 1)
        }
    }

    private func sidebarButton(
        title: String,
        icon: String,
        item: SidebarItem,
        badge: String?,
        dirtyDot: Bool
    ) -> some View {
        Button {
            selection = item
        } label: {
            HStack {
                Label(title, systemImage: icon)
                    .font(.system(size: 12.5, weight: selection == item ? .semibold : .regular))
                Spacer()
                if dirtyDot {
                    Circle()
                        .fill(Color.inAmber)
                        .frame(width: 6, height: 6)
                        .transition(.opacity)
                        .animation(.easeInOut(duration: 0.15), value: dirtyDot)
                }
                if let badge {
                    Text(badge)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundColor(.inAccent)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.inAccent.opacity(0.15))
                        .cornerRadius(4)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(selection == item ? Color.inAccent.opacity(0.15) : Color.clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundColor(selection == item ? .inAccent : .inTextSecondary)
        .accessibilityLabel(dirtyDot && item == .editor ? "Editor, unsaved changes" : title)
    }
}
