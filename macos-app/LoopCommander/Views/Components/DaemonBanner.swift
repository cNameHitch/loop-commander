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
                        .foregroundColor(.lcAmber)

                    Text("Daemon not running.")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(.lcTextPrimary)

                    Button("Start Daemon") { onStartDaemon() }
                        .buttonStyle(.borderedProminent)
                        .tint(.lcAccent)
                        .controlSize(.small)

                    Spacer()

                    Text("Some features may be unavailable")
                        .font(.system(size: 11))
                        .foregroundColor(.lcTextMuted)
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 16)
                .background(Color.lcAmber.opacity(0.1))
                .cornerRadius(LCRadius.card)
                .padding(.horizontal, 28)
                .transition(.lcFadeSlide)
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Daemon not running. Activate Start Daemon button to start.")
            }
        }
        .onChange(of: isConnected) { connected in
            if connected {
                showBanner = false
            } else {
                DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                    if !isConnected {
                        showBanner = true
                    }
                }
            }
        }
    }
}
