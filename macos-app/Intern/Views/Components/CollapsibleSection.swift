import SwiftUI

/// A uniform collapsible card section used throughout the editor settings pane.
///
/// Visual contract:
/// - Card: `Color.inSurfaceContainer` fill, `Color.inBorder` stroke at 1pt,
///   `INRadius.panel` (10pt) corner radius, 16pt internal padding.
/// - Header: chevron (9pt semibold) + uppercase label in `Font.inLabel` with
///   0.5pt tracking. The chevron rotates 90° on expand using `.inQuick`.
/// - Expand/collapse transition: `AnyTransition.inFadeSlide`.
/// - Defaults to collapsed (`isExpanded = false`).
struct CollapsibleSection<Content: View>: View {

    let title: String
    @ViewBuilder let content: () -> Content

    @State private var isExpanded: Bool = false
    @State private var headerHovered: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            headerRow

            if isExpanded {
                VStack(alignment: .leading, spacing: 12) {
                    content()
                }
                .padding(.top, 12)
                .transition(.inFadeSlide)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.panel)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.panel)
        .animation(.inFadeSlide, value: isExpanded)
    }

    // MARK: - Header Row

    private var headerRow: some View {
        Button {
            isExpanded.toggle()
        } label: {
            HStack(spacing: INSpacing.xs) {
                Image(systemName: "chevron.right")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundColor(
                        isExpanded
                            ? Color.inAccent
                            : (headerHovered ? Color.inTextSecondary : Color.inTextMuted)
                    )
                    .rotationEffect(.degrees(isExpanded ? 90 : 0))
                    .animation(.inQuick, value: isExpanded)

                Text(title.uppercased())
                    .font(.inLabel)
                    .foregroundColor(
                        isExpanded || headerHovered
                            ? Color.inTextSecondary
                            : Color.inTextMuted
                    )
                    .tracking(0.5)
                    .animation(.inQuick, value: isExpanded)
                    .animation(.inQuick, value: headerHovered)

                Spacer()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            withAnimation(.inQuick) {
                headerHovered = hovering
            }
        }
        .accessibilityLabel("\(title) section, \(isExpanded ? "expanded" : "collapsed")")
        .accessibilityHint("Activate to \(isExpanded ? "collapse" : "expand")")
        .accessibilityAddTraits(.isButton)
    }
}
