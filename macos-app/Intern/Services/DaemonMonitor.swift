import Foundation
import Combine

/// Monitors daemon connection health and provides auto-reconnect with exponential backoff.
@MainActor
class DaemonMonitor: ObservableObject {
    @Published var isConnected: Bool = false
    @Published var isReconnecting: Bool = false
    @Published var lastError: String?

    let client: DaemonClient
    private var monitorTask: Task<Void, Never>?
    private var reconnectDelay: TimeInterval = 1.0
    private let maxReconnectDelay: TimeInterval = 30.0
    private var started = false
    private var consecutiveFailures = 0
    private let maxConsecutiveFailures = 10

    init(client: DaemonClient) {
        self.client = client
    }

    /// Start monitoring the daemon connection (idempotent — safe to call multiple times)
    func start() {
        guard !started else { return }
        started = true
        monitorTask?.cancel()
        monitorTask = Task { [weak self] in
            guard let self else { return }
            await self.connectAndMonitor()
        }
    }

    /// Stop monitoring
    func stop() {
        monitorTask?.cancel()
        monitorTask = nil
        started = false
    }

    private func connectAndMonitor() async {
        while !Task.isCancelled {
            do {
                try await client.connect()
                isConnected = true
                isReconnecting = false
                lastError = nil
                reconnectDelay = 1.0
                consecutiveFailures = 0

                // Periodically check connection
                while !Task.isCancelled {
                    try await Task.sleep(nanoseconds: 10_000_000_000) // 10 seconds
                    if !client.isConnected {
                        isConnected = false
                        break
                    }
                    // Ping daemon — tolerate transient failures before disconnecting
                    do {
                        let _: DaemonStatus = try await client.call("daemon.status")
                        consecutiveFailures = 0
                    } catch DaemonClientError.timeout {
                        // Timeout means the daemon is busy but the connection may
                        // still be alive. Don't count this as a real failure.
                        continue
                    } catch DaemonClientError.rpcError {
                        // RPC errors mean the daemon responded — connection is fine.
                        consecutiveFailures = 0
                    } catch {
                        consecutiveFailures += 1
                        if consecutiveFailures >= maxConsecutiveFailures {
                            isConnected = false
                            lastError = "Daemon connection lost"
                            client.disconnect()
                            break
                        }
                        // Transient failure — don't update UI, just track internally
                    }
                }
            } catch {
                isConnected = false
                lastError = error.localizedDescription
                isReconnecting = true
            }

            // Exponential backoff reconnect
            if !Task.isCancelled {
                let delay = reconnectDelay
                reconnectDelay = min(reconnectDelay * 2, maxReconnectDelay)
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            }
        }
    }

    /// Attempt to start the daemon process
    func startDaemon() async {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let daemonPaths = [
            home.appendingPathComponent(".cargo/bin/intern").path,
            "/usr/local/bin/intern",
        ]

        var daemonPath: String?
        for path in daemonPaths {
            if FileManager.default.isExecutableFile(atPath: path) {
                daemonPath = path
                break
            }
        }

        // Also try finding via `which` (off main thread to avoid blocking UI)
        if daemonPath == nil {
            let (whichStatus, whichData): (Int32, Data) = await Task.detached {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: "/usr/bin/which")
                process.arguments = ["intern"]
                let pipe = Pipe()
                process.standardOutput = pipe
                try? process.run()
                process.waitUntilExit()
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                return (process.terminationStatus, data)
            }.value
            if whichStatus == 0 {
                if let path = String(data: whichData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    daemonPath = path
                }
            }
        }

        guard let path = daemonPath else {
            lastError = "Could not find intern binary"
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: path)
        process.arguments = ["--foreground"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            // Wait for daemon to start
            try await Task.sleep(nanoseconds: 500_000_000)
            // Reset and reconnect
            reconnectDelay = 0.1
            started = false
            start()
        } catch {
            lastError = "Failed to start daemon: \(error.localizedDescription)"
        }
    }
}
