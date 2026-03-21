import Foundation

// MARK: - JSON-RPC Types

struct JsonRpcRequest: Codable {
    let jsonrpc: String
    let method: String
    let params: AnyCodable
    let id: Int

    init(method: String, params: Any, id: Int) {
        self.jsonrpc = "2.0"
        self.method = method
        self.params = AnyCodable(params)
        self.id = id
    }
}

struct JsonRpcResponse: Codable {
    let jsonrpc: String
    let result: AnyCodable?
    let error: JsonRpcError?
    let id: Int?
}

struct JsonRpcError: Codable {
    let code: Int
    let message: String
}

/// Errors from the daemon client
enum DaemonClientError: Error, LocalizedError {
    case notConnected
    case socketError(String)
    case encodingError(String)
    case decodingError(String)
    case rpcError(code: Int, message: String)
    case timeout
    case unexpectedResponse

    var errorDescription: String? {
        switch self {
        case .notConnected:
            return "Not connected to daemon"
        case .socketError(let msg):
            return "Socket error: \(msg)"
        case .encodingError(let msg):
            return "Encoding error: \(msg)"
        case .decodingError(let msg):
            return "Decoding error: \(msg)"
        case .rpcError(let code, let message):
            return "RPC error (\(code)): \(message)"
        case .timeout:
            return "Request timed out"
        case .unexpectedResponse:
            return "Unexpected response from daemon"
        }
    }
}

// MARK: - AnyCodable wrapper for dynamic JSON values

struct AnyCodable: Codable {
    let value: Any

    init(_ value: Any) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Unsupported type")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            if let jsonData = try? JSONSerialization.data(withJSONObject: value),
               let jsonString = String(data: jsonData, encoding: .utf8) {
                try container.encode(jsonString)
            } else {
                try container.encodeNil()
            }
        }
    }
}

// MARK: - Daemon Client

/// Thread-safe client for communicating with the Loop Commander daemon via Unix socket.
/// All socket I/O is performed on a background queue to avoid blocking the main thread.
final class DaemonClient: @unchecked Sendable {
    private let queue = DispatchQueue(label: "com.loopcommander.client", qos: .userInitiated)
    private var socketFD: Int32 = -1
    private var requestId: Int = 0
    private let lock = NSLock()
    private let socketPath: String
    private var _isConnected: Bool = false

    var isConnected: Bool {
        lock.lock()
        defer { lock.unlock() }
        return _isConnected
    }

    init() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        socketPath = home.appendingPathComponent(".loop-commander/daemon.sock").path
    }

    init(socketPath: String) {
        self.socketPath = socketPath
    }

    // MARK: - Connection Management

    func connect() async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async { [self] in
                do {
                    try self.connectSync()
                    continuation.resume()
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private func connectSync() throws {
        lock.lock()
        if _isConnected {
            lock.unlock()
            return
        }
        lock.unlock()

        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw DaemonClientError.socketError("Failed to create socket: \(String(cString: strerror(errno)))")
        }

        // Set socket read timeout to 30 seconds.
        // The daemon may be busy handling task.run_now or other long operations,
        // so 10s was too aggressive and caused spurious disconnects.
        var timeout = timeval(tv_sec: 30, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))

        // Build sockaddr_un
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        addr.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let pathBytes = socketPath.utf8CString
        guard pathBytes.count <= MemoryLayout.size(ofValue: addr.sun_path) else {
            Darwin.close(fd)
            throw DaemonClientError.socketError("Socket path too long")
        }

        withUnsafeMutablePointer(to: &addr.sun_path) { sunPathPtr in
            sunPathPtr.withMemoryRebound(to: CChar.self, capacity: pathBytes.count) { ptr in
                for (i, byte) in pathBytes.enumerated() {
                    ptr[i] = byte
                }
            }
        }

        let result = withUnsafePointer(to: &addr) { addrPtr in
            addrPtr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                Darwin.connect(fd, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }

        guard result == 0 else {
            Darwin.close(fd)
            throw DaemonClientError.socketError("Failed to connect: \(String(cString: strerror(errno)))")
        }

        lock.lock()
        socketFD = fd
        _isConnected = true
        lock.unlock()
    }

    func disconnect() {
        lock.lock()
        if socketFD >= 0 {
            Darwin.close(socketFD)
            socketFD = -1
        }
        _isConnected = false
        lock.unlock()
    }

    // MARK: - RPC Call (non-blocking via background queue)

    func call<T: Decodable>(_ method: String, params: Any = [:] as [String: Any]) async throws -> T {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<T, Error>) in
            queue.async { [self] in
                do {
                    let result: T = try self.callSync(method, params: params)
                    continuation.resume(returning: result)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func callVoid(_ method: String, params: Any = [:] as [String: Any]) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async { [self] in
                do {
                    try self.callVoidSync(method, params: params)
                    continuation.resume()
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    // MARK: - Synchronous I/O (runs on background queue only)

    private func callSync<T: Decodable>(_ method: String, params: Any) throws -> T {
        do {
            return try doCallSync(method, params: params)
        } catch DaemonClientError.socketError, DaemonClientError.notConnected {
            // Connection went stale — try reconnecting once.
            guard reconnectSync() else {
                throw DaemonClientError.notConnected
            }
            return try doCallSync(method, params: params)
        }
    }

    private func callVoidSync(_ method: String, params: Any) throws {
        do {
            try doCallVoidSync(method, params: params)
        } catch DaemonClientError.socketError, DaemonClientError.notConnected {
            guard reconnectSync() else {
                throw DaemonClientError.notConnected
            }
            try doCallVoidSync(method, params: params)
        }
    }

    private func doCallSync<T: Decodable>(_ method: String, params: Any) throws -> T {
        lock.lock()
        guard _isConnected, socketFD >= 0 else {
            lock.unlock()
            throw DaemonClientError.notConnected
        }
        let fd = socketFD
        requestId += 1
        let currentId = requestId
        lock.unlock()

        // Build request
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": currentId
        ]

        guard let requestData = try? JSONSerialization.data(withJSONObject: request),
              var requestString = String(data: requestData, encoding: .utf8) else {
            throw DaemonClientError.encodingError("Failed to serialize request")
        }

        requestString += "\n"

        guard let sendData = requestString.data(using: .utf8) else {
            throw DaemonClientError.encodingError("Failed to encode request string")
        }

        // Write to socket
        let written = sendData.withUnsafeBytes { ptr in
            Darwin.write(fd, ptr.baseAddress!, sendData.count)
        }
        guard written == sendData.count else {
            markDisconnected()
            throw DaemonClientError.socketError("Write failed")
        }

        // Read response line
        let responseData = try readLineSync(fd: fd)

        guard !responseData.isEmpty else {
            markDisconnected()
            throw DaemonClientError.socketError("Empty response")
        }

        // Parse response
        guard let responseJSON = try JSONSerialization.jsonObject(with: responseData) as? [String: Any] else {
            throw DaemonClientError.decodingError("Invalid JSON response")
        }

        if let errorObj = responseJSON["error"] as? [String: Any],
           let code = errorObj["code"] as? Int,
           let message = errorObj["message"] as? String {
            throw DaemonClientError.rpcError(code: code, message: message)
        }

        guard let result = responseJSON["result"] else {
            throw DaemonClientError.unexpectedResponse
        }

        let resultData = try JSONSerialization.data(withJSONObject: result)
        let decoder = JSONDecoder()
        return try decoder.decode(T.self, from: resultData)
    }

    private func doCallVoidSync(_ method: String, params: Any) throws {
        lock.lock()
        guard _isConnected, socketFD >= 0 else {
            lock.unlock()
            throw DaemonClientError.notConnected
        }
        let fd = socketFD
        requestId += 1
        let currentId = requestId
        lock.unlock()

        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": currentId
        ]

        guard let requestData = try? JSONSerialization.data(withJSONObject: request),
              var requestString = String(data: requestData, encoding: .utf8) else {
            throw DaemonClientError.encodingError("Failed to serialize request")
        }

        requestString += "\n"

        guard let sendData = requestString.data(using: .utf8) else {
            throw DaemonClientError.encodingError("Failed to encode request string")
        }

        let written = sendData.withUnsafeBytes { ptr in
            Darwin.write(fd, ptr.baseAddress!, sendData.count)
        }
        guard written == sendData.count else {
            markDisconnected()
            throw DaemonClientError.socketError("Write failed")
        }

        let responseData = try readLineSync(fd: fd)

        guard !responseData.isEmpty else {
            markDisconnected()
            throw DaemonClientError.socketError("Empty response")
        }

        guard let responseJSON = try JSONSerialization.jsonObject(with: responseData) as? [String: Any] else {
            throw DaemonClientError.decodingError("Invalid JSON response")
        }

        if let errorObj = responseJSON["error"] as? [String: Any],
           let code = errorObj["code"] as? Int,
           let message = errorObj["message"] as? String {
            throw DaemonClientError.rpcError(code: code, message: message)
        }
    }

    /// Read bytes from the file descriptor until a newline is found.
    /// Uses raw `Darwin.read()` with the socket's SO_RCVTIMEO timeout.
    private func readLineSync(fd: Int32) throws -> Data {
        var buffer = Data()
        let newline = UInt8(ascii: "\n")
        var readBuf = [UInt8](repeating: 0, count: 4096)

        while true {
            let n = Darwin.read(fd, &readBuf, readBuf.count)
            if n < 0 {
                let err = errno
                // EINTR: interrupted by signal — just retry the read.
                if err == EINTR {
                    continue
                }
                // EAGAIN/EWOULDBLOCK/ETIMEDOUT: socket timeout expired.
                // This is transient — don't mark disconnected.
                if err == EAGAIN || err == EWOULDBLOCK || err == ETIMEDOUT {
                    throw DaemonClientError.timeout
                }
                markDisconnected()
                throw DaemonClientError.socketError("Read failed: \(String(cString: strerror(err)))")
            }
            if n == 0 {
                markDisconnected()
                if buffer.isEmpty {
                    throw DaemonClientError.socketError("Connection closed")
                }
                return buffer
            }

            let chunk = Data(readBuf[0..<n])
            if let nlIndex = chunk.firstIndex(of: newline) {
                buffer.append(chunk[chunk.startIndex..<nlIndex])
                return buffer
            } else {
                buffer.append(chunk)
            }

            if buffer.count > 10_000_000 {
                throw DaemonClientError.decodingError("Response too large")
            }
        }
    }

    private func markDisconnected() {
        lock.lock()
        if socketFD >= 0 {
            Darwin.close(socketFD)
            socketFD = -1
        }
        _isConnected = false
        lock.unlock()
    }

    /// Attempt to reconnect and retry a call once. Used by callSync/callVoidSync
    /// to transparently recover from stale connections.
    private func reconnectSync() -> Bool {
        lock.lock()
        if socketFD >= 0 {
            Darwin.close(socketFD)
            socketFD = -1
        }
        _isConnected = false
        lock.unlock()
        do {
            try connectSync()
            return true
        } catch {
            return false
        }
    }

    // MARK: - Convenience Methods

    func listTasks() async throws -> [LCTask] {
        return try await call("task.list")
    }

    func getTask(_ id: String) async throws -> LCTask {
        return try await call("task.get", params: ["id": id])
    }

    func createTask(_ params: [String: Any]) async throws -> LCTask {
        return try await call("task.create", params: params)
    }

    func updateTask(_ params: [String: Any]) async throws -> LCTask {
        return try await call("task.update", params: params)
    }

    func deleteTask(_ id: String) async throws {
        try await callVoid("task.delete", params: ["id": id])
    }

    func pauseTask(_ id: String) async throws -> LCTask {
        return try await call("task.pause", params: ["id": id])
    }

    func resumeTask(_ id: String) async throws -> LCTask {
        return try await call("task.resume", params: ["id": id])
    }

    func runTaskNow(_ id: String) async throws {
        try await callVoid("task.run_now", params: ["id": id])
    }

    func dryRunTask(_ id: String) async throws -> DryRunResult {
        return try await call("task.dry_run", params: ["id": id])
    }

    func stopTask(_ id: String) async throws {
        try await callVoid("task.stop", params: ["id": id])
    }

    func queryLogs(_ query: LogQuery) async throws -> [ExecutionLog] {
        var params: [String: Any] = [:]
        if let taskId = query.taskId { params["task_id"] = taskId }
        if let status = query.status { params["status"] = status }
        if let limit = query.limit { params["limit"] = limit }
        if let offset = query.offset { params["offset"] = offset }
        if let search = query.search { params["search"] = search }
        return try await call("logs.query", params: params)
    }

    func getDashboardMetrics() async throws -> DashboardMetrics {
        return try await call("metrics.dashboard")
    }

    func getDaemonStatus() async throws -> DaemonStatus {
        return try await call("daemon.status")
    }

    func getTemplates() async throws -> [TaskTemplate] {
        return try await call("templates.list")
    }

    func exportTask(_ id: String) async throws -> TaskExport {
        return try await call("task.export", params: ["id": id])
    }

    func importTask(_ export: TaskExport) async throws -> LCTask {
        let encoder = JSONEncoder()
        let data = try encoder.encode(export)
        guard let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw DaemonClientError.encodingError("Failed to encode export")
        }
        return try await call("task.import", params: dict)
    }

    func getCostTrend(days: Int = 7) async throws -> [DailyCost] {
        return try await call("metrics.cost_trend", params: ["days": days])
    }

    func listAgents() async throws -> [AgentEntry] {
        return try await call("registry.list")
    }

    func refreshAgentRegistry() async throws -> RegistryRefreshResult {
        return try await call("registry.refresh")
    }

    /// Generate a prompt from a natural-language intent and a set of agent slugs.
    ///
    /// NOTE: This call invokes an LLM on the daemon side and may take up to 60 seconds.
    /// The socket read timeout is set to 30 seconds by default, so callers should be
    /// aware that this method may hit DaemonClientError.timeout on slow networks or
    /// heavily loaded daemons.
    func generatePrompt(intent: String, agents: [String], workingDir: String) async throws -> PromptGenerateResult {
        return try await call("prompt.generate", params: [
            "intent": intent,
            "agents": agents,
            "working_dir": workingDir
        ])
    }
}
