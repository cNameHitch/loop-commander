import SwiftUI

/// Banner shown when the daemon is not running.
/// Delays appearance by 2 seconds to avoid flicker on transient disconnects.
struct DaemonBanner: View {
    let isConnected: Bool
    let onStartDaemon: () -> Void
    @State private var showBanner = false

    var body: some View {
        Group {
            if showBanner {
                HStack(spacing: 10) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.inAmber)

                    Text("Intern is offline.")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(.inTextPrimary)

                    Button("Wake Up Intern") { onStartDaemon() }
                        .buttonStyle(.borderedProminent)
                        .tint(.inAccent)
                        .controlSize(.small)

                    Spacer()

                    Text("Tasks will not run until Intern is active")
                        .font(.system(size: 11))
                        .foregroundColor(.inTextMuted)
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 16)
                .background(Color.inAmber.opacity(0.1))
                .cornerRadius(INRadius.card)
                .padding(.horizontal, 28)
                .transition(.inFadeSlide)
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Intern is offline. Activate Wake Up Intern button to start.")
            }
        }
        .onChange(of: isConnected) { connected in
            if connected {
                showBanner = false
            } else {
                DispatchQueue.main.asyncAfter(deadline: .now() + 5.0) {
                    if !isConnected {
                        showBanner = true
                    }
                }
            }
        }
    }
}
