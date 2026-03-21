import Foundation
import Combine

/// ViewModel for the global logs view.
@MainActor
class LogsViewModel: ObservableObject {
    @Published var logs: [ExecutionLog] = []
    @Published var isLoading: Bool = false
    @Published var error: String?
    @Published var searchQuery: String = ""
    @Published var filter: LogFilter = .all
    @Published var expandedLogIds: Set<Int> = []

    private var client: DaemonClient?
    private var searchDebounce: AnyCancellable?

    func setClient(_ client: DaemonClient) {
        self.client = client
    }

    func setupSearchDebounce() {
        searchDebounce = $searchQuery
            .debounce(for: .milliseconds(300), scheduler: RunLoop.main)
            .removeDuplicates()
            .sink { [weak self] _ in
                Task { [weak self] in
                    await self?.loadLogs()
                }
            }
    }

    func loadLogs() async {
        guard let client = client else { return }
        isLoading = true
        error = nil

        var query = LogQuery(limit: 100)
        if filter != .all {
            query.status = filter.rawValue
        }
        if !searchQuery.isEmpty {
            query.search = searchQuery
        }

        do {
            logs = try await client.queryLogs(query)
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func toggleExpanded(_ logId: Int) {
        if expandedLogIds.contains(logId) {
            expandedLogIds.remove(logId)
        } else {
            expandedLogIds.insert(logId)
        }
    }

    func isExpanded(_ logId: Int) -> Bool {
        expandedLogIds.contains(logId)
    }
}
