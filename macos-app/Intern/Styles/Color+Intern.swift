import SwiftUI

// MARK: - Hex Initializer

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let r, g, b: Double
        if hex.count == 6 {
            r = Double((int >> 16) & 0xFF) / 255.0
            g = Double((int >> 8) & 0xFF) / 255.0
            b = Double(int & 0xFF) / 255.0
        } else {
            r = 1; g = 1; b = 1
        }
        self.init(red: r, green: g, blue: b)
    }
}

// MARK: - Adaptive Initializer

extension Color {
    /// Creates a color that resolves to `dark` in Dark Aqua appearances
    /// and `light` in all other appearances (Aqua, High Contrast Aqua, etc.).
    /// Uses NSColor's appearance block — the same mechanism SwiftUI uses for
    /// semantic colors — so it responds to `.preferredColorScheme` modifiers.
    init(light: Color, dark: Color) {
        self.init(NSColor(name: nil) { appearance in
            let resolved = appearance.bestMatch(from: [.aqua, .darkAqua])
            return resolved == .darkAqua ? NSColor(dark) : NSColor(light)
        })
    }
}

// MARK: - Design Tokens

extension Color {
    // -- Backgrounds --
    /// App root background. Dark: #0f1117 / Light: #f0f2f5
    static let inBackground = Color(
        light: Color(hex: "f0f2f5"),
        dark:  Color(hex: "0f1117")
    )
    /// Modal / editor panel background. Dark: #1a1d23 / Light: #ffffff
    static let inSurface = Color(
        light: Color(hex: "ffffff"),
        dark:  Color(hex: "1a1d23")
    )
    /// Subtle raised surface. Dark: white.opacity(0.02) / Light: black.opacity(0.02)
    static let inSurfaceRaised = Color(
        light: Color.black.opacity(0.02),
        dark:  Color.white.opacity(0.02)
    )
    /// Card / table container. Dark: white.opacity(0.01) / Light: black.opacity(0.03)
    static let inSurfaceContainer = Color(
        light: Color.black.opacity(0.03),
        dark:  Color.white.opacity(0.01)
    )
    /// Code block background. Dark: black.opacity(0.3) / Light: black.opacity(0.06)
    static let inCodeBackground = Color(
        light: Color.black.opacity(0.06),
        dark:  Color.black.opacity(0.3)
    )

    // -- Text --
    /// Primary text. Dark: #e2e8f0 / Light: #1a1d23
    static let inTextPrimary = Color(
        light: Color(hex: "1a1d23"),
        dark:  Color(hex: "e2e8f0")
    )
    /// Secondary / detail text. Dark: #c8d0dc / Light: #3d4555
    static let inTextSecondary = Color(
        light: Color(hex: "3d4555"),
        dark:  Color(hex: "c8d0dc")
    )
    /// Muted text (labels, timestamps). Dark: white.opacity(0.4) / Light: black.opacity(0.45)
    static let inTextMuted = Color(
        light: Color.black.opacity(0.45),
        dark:  Color.white.opacity(0.4)
    )
    /// Very muted text (sublabels, working dirs). Dark: white.opacity(0.35) / Light: black.opacity(0.38)
    static let inTextSubtle = Color(
        light: Color.black.opacity(0.38),
        dark:  Color.white.opacity(0.35)
    )
    /// Faintest text (column headers). Dark: white.opacity(0.3) / Light: black.opacity(0.30)
    static let inTextFaint = Color(
        light: Color.black.opacity(0.30),
        dark:  Color.white.opacity(0.3)
    )
    /// Dimmest text (log filter inactive). Dark: white.opacity(0.25) / Light: black.opacity(0.25)
    static let inTextDimmest = Color(
        light: Color.black.opacity(0.25),
        dark:  Color.white.opacity(0.25)
    )

    // -- Accents --
    /// Primary accent (indigo). Dark: #818cf8 / Light: #4f46e5
    static let inAccent = Color(
        light: Color(hex: "4f46e5"),
        dark:  Color(hex: "818cf8")
    )
    /// Accent pressed / gradient end. Dark: #6366f1 / Light: #3730a3
    static let inAccentDeep = Color(
        light: Color(hex: "3730a3"),
        dark:  Color(hex: "6366f1")
    )
    /// Accent for active text / links. Dark: #a5b4fc / Light: #4338ca
    static let inAccentLight = Color(
        light: Color(hex: "4338ca"),
        dark:  Color(hex: "a5b4fc")
    )
    /// Accent background wash. Dark: #818cf8.opacity(0.15) / Light: #4f46e5.opacity(0.10)
    static let inAccentBg = Color(
        light: Color(hex: "4f46e5").opacity(0.10),
        dark:  Color(hex: "818cf8").opacity(0.15)
    )
    /// Accent background subtle (selected row). Dark: #6366f1.opacity(0.08) / Light: #4f46e5.opacity(0.06)
    static let inAccentBgSubtle = Color(
        light: Color(hex: "4f46e5").opacity(0.06),
        dark:  Color(hex: "6366f1").opacity(0.08)
    )
    /// Tag background. Dark: #818cf8.opacity(0.1) / Light: #4f46e5.opacity(0.08)
    static let inTagBg = Color(
        light: Color(hex: "4f46e5").opacity(0.08),
        dark:  Color(hex: "818cf8").opacity(0.1)
    )
    /// Focus ring / input focus. Dark: #818cf8.opacity(0.5) / Light: #4f46e5.opacity(0.4)
    static let inAccentFocus = Color(
        light: Color(hex: "4f46e5").opacity(0.4),
        dark:  Color(hex: "818cf8").opacity(0.5)
    )

    // -- Status: Active / Success --
    /// Green. Dark: #22c55e / Light: #16a34a
    static let inGreen = Color(
        light: Color(hex: "16a34a"),
        dark:  Color(hex: "22c55e")
    )
    /// Green background wash. Dark: #22c55e.opacity(0.1) / Light: #16a34a.opacity(0.12)
    static let inGreenBg = Color(
        light: Color(hex: "16a34a").opacity(0.12),
        dark:  Color(hex: "22c55e").opacity(0.1)
    )
    /// Green background subtle (log success). Dark: #22c55e.opacity(0.08) / Light: #16a34a.opacity(0.10)
    static let inGreenBgSubtle = Color(
        light: Color(hex: "16a34a").opacity(0.10),
        dark:  Color(hex: "22c55e").opacity(0.08)
    )

    // -- Status: Paused / Warning --
    /// Amber. Dark: #f59e0b / Light: #d97706
    static let inAmber = Color(
        light: Color(hex: "d97706"),
        dark:  Color(hex: "f59e0b")
    )
    /// Amber background wash. Dark: #f59e0b.opacity(0.1) / Light: #d97706.opacity(0.12)
    static let inAmberBg = Color(
        light: Color(hex: "d97706").opacity(0.12),
        dark:  Color(hex: "f59e0b").opacity(0.1)
    )

    // -- Status: Error --
    /// Red. Dark: #ef4444 / Light: #dc2626
    static let inRed = Color(
        light: Color(hex: "dc2626"),
        dark:  Color(hex: "ef4444")
    )
    /// Red background wash. Dark: #ef4444.opacity(0.1) / Light: #dc2626.opacity(0.12)
    static let inRedBg = Color(
        light: Color(hex: "dc2626").opacity(0.12),
        dark:  Color(hex: "ef4444").opacity(0.1)
    )
    /// Red border for delete button. Dark: #ef4444.opacity(0.2) / Light: #dc2626.opacity(0.25)
    static let inRedBorder = Color(
        light: Color(hex: "dc2626").opacity(0.25),
        dark:  Color(hex: "ef4444").opacity(0.2)
    )

    // -- Borders & Separators --
    /// Standard border. Dark: white.opacity(0.06) / Light: black.opacity(0.10)
    static let inBorder = Color(
        light: Color.black.opacity(0.10),
        dark:  Color.white.opacity(0.06)
    )
    /// Input border. Dark: white.opacity(0.1) / Light: black.opacity(0.15)
    static let inBorderInput = Color(
        light: Color.black.opacity(0.15),
        dark:  Color.white.opacity(0.1)
    )
    /// Divider (thinner). Dark: white.opacity(0.04) / Light: black.opacity(0.08)
    static let inDivider = Color(
        light: Color.black.opacity(0.08),
        dark:  Color.white.opacity(0.04)
    )
    /// Header divider / toolbar separator. Dark: white.opacity(0.08) / Light: black.opacity(0.12)
    static let inSeparator = Color(
        light: Color.black.opacity(0.12),
        dark:  Color.white.opacity(0.08)
    )
    /// Scrollbar thumb. Dark: white.opacity(0.08) / Light: black.opacity(0.15)
    static let inScrollbar = Color(
        light: Color.black.opacity(0.15),
        dark:  Color.white.opacity(0.08)
    )
    /// Scrollbar thumb hover. Dark: white.opacity(0.15) / Light: black.opacity(0.25)
    static let inScrollbarHover = Color(
        light: Color.black.opacity(0.25),
        dark:  Color.white.opacity(0.15)
    )

    // -- Selected Row --
    /// Selected row left border. Dark: #818cf8 / Light: #4f46e5
    static let inSelectedBorder = Color(
        light: Color(hex: "4f46e5"),
        dark:  Color(hex: "818cf8")
    )

    // -- Overlay --
    /// Modal backdrop. Dark: black.opacity(0.7) / Light: black.opacity(0.5)
    static let inOverlay = Color(
        light: Color.black.opacity(0.5),
        dark:  Color.black.opacity(0.7)
    )
}

// MARK: - Health Color

extension Color {
    /// Returns the appropriate health color for a success rate percentage.
    /// JSX: >= 95 -> green, >= 80 -> amber, < 80 -> red
    static func inHealthColor(for successRate: Int) -> Color {
        if successRate >= 95 { return .inGreen }
        if successRate >= 80 { return .inAmber }
        return .inRed
    }
}
