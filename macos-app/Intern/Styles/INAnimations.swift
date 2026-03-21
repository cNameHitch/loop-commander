import SwiftUI

extension Animation {
    /// General UI transition. JSX: transition "all 0.15s ease"
    static let inQuick = Animation.easeInOut(duration: 0.15)
    /// Expand/collapse, modal appear. JSX: @keyframes fadeSlide 0.2s ease
    static let inFadeSlide = Animation.easeOut(duration: 0.2)
    /// Running-task pulse. JSX: @keyframes pulse 0.5->1.0 opacity
    static let inPulse = Animation.easeInOut(duration: 1.0).repeatForever(autoreverses: true)
}

// fadeSlide transition: opacity 0->1, translateY -4->0
extension AnyTransition {
    static var inFadeSlide: AnyTransition {
        .asymmetric(
            insertion: .opacity.combined(with: .move(edge: .top)).animation(.inFadeSlide),
            removal: .opacity.animation(.inQuick)
        )
    }
}

// Modal shadow
extension View {
    /// Editor modal shadow. JSX: 0 24px 80px rgba(0,0,0,0.6)
    func inModalShadow() -> some View {
        self.shadow(color: .black.opacity(0.6), radius: 40, x: 0, y: 24)
    }
}

// MARK: - Conditional Rotation for Running Status

struct ConditionalRotation: ViewModifier {
    let animate: Bool
    @State private var isRotating = false

    func body(content: Content) -> some View {
        content
            .rotationEffect(.degrees(isRotating && animate ? 360 : 0))
            .animation(
                animate ? .linear(duration: 2).repeatForever(autoreverses: false) : .default,
                value: isRotating
            )
            .onAppear { isRotating = true }
    }
}

// MARK: - INBorder

enum INBorder {
    /// 1px - Standard borders on cards, panels, inputs, dividers
    static let standard: CGFloat = 1
    /// 2px - Selected row left accent border
    static let selected: CGFloat = 2
}
