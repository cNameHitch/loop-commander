import SwiftUI

struct INPrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.inButton)
            .foregroundColor(.white)
            .padding(.vertical, 10)
            .padding(.horizontal, 24)
            .background(Color.inAccent)
            .cornerRadius(INRadius.button)
            .opacity(configuration.isPressed ? 0.8 : 1.0)
    }
}

struct INSecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .medium))
            .foregroundColor(.inTextMuted)
            .padding(.vertical, 10)
            .padding(.horizontal, 20)
            .background(Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: 1)
            )
            .cornerRadius(INRadius.button)
            .opacity(configuration.isPressed ? 0.7 : 1.0)
    }
}

struct INDangerButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.inButtonSmall)
            .foregroundColor(.inRed)
            .padding(.vertical, 6)
            .padding(.horizontal, 14)
            .background(Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inRedBorder, lineWidth: 1)
            )
            .cornerRadius(INRadius.button)
            .opacity(configuration.isPressed ? 0.7 : 1.0)
    }
}

struct INToolbarButtonStyle: ButtonStyle {
    var foreground: Color = .inTextMuted

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.inButtonSmall)
            .foregroundColor(foreground)
            .padding(.vertical, 6)
            .padding(.horizontal, 14)
            .background(Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: 1)
            )
            .cornerRadius(INRadius.button)
            .opacity(configuration.isPressed ? 0.7 : 1.0)
    }
}
