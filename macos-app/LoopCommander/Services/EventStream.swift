import Foundation
import Combine

/// Subscribes to the daemon event stream via `events.subscribe`.
/// Holds the connection open and receives newline-delimited JSON events.
/// All socket I/O runs on a background queue.
@MainActor
class EventStream: ObservableObject {
    @Published var lastEvent: DaemonEvent?
    @Published var isSubscribed: Bool = false

    private let socketPath: String
    private var streamTask: Task<Void, Never>?
    private let ioQueue = DispatchQueue(label: "com.loopcommander.eventstream", qos: .utility)
    private var reconnectDelay: TimeInterval = 1.0
    private let maxReconnectDelay: TimeInterval = 30.0
    private var started = false

    /// Callbacks for specific event types
    var onTaskStarted: ((String, String) -> Void)?
    var onTaskCompleted: ((String, String, Int, Double?) -> Void)?
    var onTaskFailed: ((String, String, Int, String) -> Void)?
    var onTaskStatusChanged: ((String) -> Void)?
    var onAnyEvent: (() -> Void)?

    init() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        socketPath = home.appendingPathComponent(".loop-commander/daemon.sock").path
    }

    /// Start the event subscription (idempotent)
    func start() {
        guard !started else { return }
        started = true
        streamTask?.cancel()
        streamTask = Task { [weak self] in
            guard let self else { return }
            await self.subscribeLoop()
        }
    }

    /// Stop the event subscription
    func stop() {
        streamTask?.cancel()
        streamTask = nil
        started = false
        isSubscribed = false
    }

    private func subscribeLoop() async {
        while !Task.isCancelled {
            do {
                try await subscribe()
            } catch {
                await MainActor.run { isSubscribed = false }
            }

            // Reconnect with backoff
            if !Task.isCancelled {
                let delay = reconnectDelay
                reconnectDelay = min(reconnectDelay * 2, maxReconnectDelay)
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            }
        }
    }

    private func subscribe() async throws {
        // Run all blocking socket I/O on background queue
        let fd = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Int32, Error>) in
            ioQueue.async { [self] in
                do {
                    let fd = try self.connectAndSubscribe()
                    continuation.resume(returning: fd)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }

        defer {
            ioQueue.async { Darwin.close(fd) }
        }

        isSubscribed = true
        reconnectDelay = 1.0

        // Read events on background queue, dispatch to main
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            ioQueue.async { [weak self] in
                guard let self else {
                    continuation.resume()
                    return
                }
                self.readEvents(fd: fd)
                continuation.resume()
            }
        }
    }

    /// Blocking connect + subscribe (runs on ioQueue)
    private nonisolated func connectAndSubscribe() throws -> Int32 {
        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw DaemonClientError.socketError("Failed to create socket")
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        addr.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let pathBytes = socketPath.utf8CString
        withUnsafeMutablePointer(to: &addr.sun_path) { sunPathPtr in
            sunPathPtr.withMemoryRebound(to: CChar.self, capacity: pathBytes.count) { ptr in
                for (i, byte) in pathBytes.enumerated() {
                    ptr[i] = byte
                }
            }
        }

        let connectResult = withUnsafePointer(to: &addr) { addrPtr in
            addrPtr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                Darwin.connect(fd, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }

        guard connectResult == 0 else {
            Darwin.close(fd)
            throw DaemonClientError.socketError("Failed to connect for events")
        }

        // Send subscribe request
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "method": "events.subscribe",
            "params": [:] as [String: Any],
            "id": 1
        ]

        guard let requestData = try? JSONSerialization.data(withJSONObject: request),
              var requestString = String(data: requestData, encoding: .utf8) else {
            Darwin.close(fd)
            throw DaemonClientError.encodingError("Failed to serialize subscribe request")
        }

        requestString += "\n"

        guard let sendData = requestString.data(using: .utf8) else {
            Darwin.close(fd)
            throw DaemonClientError.encodingError("Failed to encode subscribe request")
        }

        let written = sendData.withUnsafeBytes { ptr in
            Darwin.write(fd, ptr.baseAddress!, sendData.count)
        }
        guard written == sendData.count else {
            Darwin.close(fd)
            throw DaemonClientError.socketError("Write failed")
        }

        return fd
    }

    /// Blocking event read loop (runs on ioQueue)
    private nonisolated func readEvents(fd: Int32) {
        var buffer = Data()
        let newline = UInt8(ascii: "\n")
        var readBuf = [UInt8](repeating: 0, count: 4096)

        while true {
            let n = Darwin.read(fd, &readBuf, readBuf.count)
            if n <= 0 {
                // Connection closed or error
                Task { @MainActor [weak self] in
                    self?.isSubscribed = false
                }
                return
            }

            buffer.append(Data(readBuf[0..<n]))

            // Process complete lines
            while let nlIndex = buffer.firstIndex(of: newline) {
                let lineData = Data(buffer[buffer.startIndex..<nlIndex])
                buffer = Data(buffer[(nlIndex + 1)...])

                if lineData.isEmpty { continue }

                do {
                    if let json = try JSONSerialization.jsonObject(with: lineData) as? [String: Any] {
                        // Skip initial subscription response (has "id" field)
                        if json["id"] != nil { continue }

                        if let paramsData = json["params"] {
                            let eventData = try JSONSerialization.data(withJSONObject: paramsData)
                            let event = try JSONDecoder().decode(DaemonEvent.self, from: eventData)
                            Task { @MainActor [weak self] in
                                self?.lastEvent = event
                                if let self { self.dispatchEvent(event) }
                            }
                        }
                    }
                } catch {
                    continue
                }
            }
        }
    }

    private func dispatchEvent(_ event: DaemonEvent) {
        onAnyEvent?()

        switch event {
        case .taskStarted(let taskId, let taskName):
            onTaskStarted?(taskId, taskName)
        case .taskCompleted(let taskId, let taskName, let durationSecs, let costUsd):
            onTaskCompleted?(taskId, taskName, durationSecs, costUsd)
        case .taskFailed(let taskId, let taskName, let exitCode, let summary):
            onTaskFailed?(taskId, taskName, exitCode, summary)
        case .taskStatusChanged(let taskId, _, _):
            onTaskStatusChanged?(taskId)
        default:
            break
        }
    }
}
