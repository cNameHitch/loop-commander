import Foundation
import Combine

/// ViewModel for dashboard metrics. Refreshes on appear, every 30 seconds, and on events.
@MainActor
class DashboardViewModel: ObservableObject {
    @Published var metrics: DashboardMetrics = .empty
    @Published var isLoading: Bool = false
    @Published var error: String?

    var activeCount: Int { metrics.activeTasks }

    private var refreshTimer: AnyCancellable?
    private var client: DaemonClient?

    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    func loadMetrics() async {
        guard let client = client else { return }
        isLoading = true
        error = nil

        do {
            metrics = try await client.getDashboardMetrics()

            // Also try to get cost trend
            do {
                let trend = try await client.getCostTrend(days: 7)
                metrics.costTrend = trend
            } catch {
                // Cost trend is optional
            }

            // Get daemon PID
            do {
                let status: DaemonStatus = try await client.getDaemonStatus()
                metrics.daemonPID = status.pid
            } catch {
                // Daemon status is optional for metrics
            }
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func startRefreshTimer() {
        refreshTimer?.cancel()
        refreshTimer = Timer.publish(every: 30, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                Task { [weak self] in
                    await self?.loadMetrics()
                }
            }
    }

    func stopRefreshTimer() {
        refreshTimer?.cancel()
        refreshTimer = nil
    }
}
