import SwiftUI

struct MetricCard: View {
    let label: String
    let value: String
    var sub: String? = nil
    var accent: Color = .inTextPrimary

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(label)
                .font(.inMetricLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)
                .lineLimit(1)
                .padding(.bottom, 6)

            Text(value)
                .font(.inMetricValue)
                .foregroundColor(accent)
                .lineLimit(1)
                .minimumScaleFactor(0.5)

            if let sub = sub {
                Text(sub)
                    .font(.inMetricSub)
                    .foregroundColor(.inTextSubtle)
                    .padding(.top, 6)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 88, alignment: .leading)
        .padding(.vertical, 18)
        .padding(.horizontal, 20)
        .background(Color.inSurfaceRaised)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.card)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.card)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(label): \(value)")
        .accessibilityValue(sub ?? "")
    }
}
