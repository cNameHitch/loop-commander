import SwiftUI
import Charts

struct SparklineChart: View {
    let data: [DailyCost]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("7-Day Spend")
                .font(.inMetricLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)
                .padding(.bottom, 6)

            if #available(macOS 14.0, *) {
                chartView
            } else {
                // Fallback for macOS 13: simple text sparkline
                textSparkline
            }

            let total = data.reduce(0) { $0 + $1.totalCost }
            Text("$\(String(format: "%.2f", total)) total")
                .font(.inMetricSub)
                .foregroundColor(.inTextSubtle)
                .padding(.top, 6)
        }
        .frame(maxWidth: .infinity, minHeight: 88)
        .padding(.vertical, 18)
        .padding(.horizontal, 20)
        .background(Color.inSurfaceRaised)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.card)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.card)
    }

    @available(macOS 14.0, *)
    private var chartView: some View {
        Chart(data) { entry in
            LineMark(
                x: .value("Date", entry.date),
                y: .value("Cost", entry.totalCost)
            )
            .foregroundStyle(Color.inAccent)
            .interpolationMethod(.catmullRom)

            AreaMark(
                x: .value("Date", entry.date),
                y: .value("Cost", entry.totalCost)
            )
            .foregroundStyle(
                LinearGradient(
                    colors: [Color.inAccent.opacity(0.3), Color.inAccent.opacity(0.0)],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .interpolationMethod(.catmullRom)
        }
        .chartXAxis(.hidden)
        .chartYAxis(.hidden)
        .frame(height: 48)
    }

    private var textSparkline: some View {
        HStack(spacing: 1) {
            let maxCost = data.map(\.totalCost).max() ?? 1.0
            ForEach(data) { entry in
                let normalizedHeight = maxCost > 0 ? entry.totalCost / maxCost : 0
                let barChars = ["\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}", "\u{2585}", "\u{2586}", "\u{2587}", "\u{2588}"]
                let idx = min(Int(normalizedHeight * 7), 7)
                Text(barChars[idx])
                    .font(.system(size: 20))
                    .foregroundColor(.inAccent)
            }
        }
        .frame(height: 48)
    }
}
