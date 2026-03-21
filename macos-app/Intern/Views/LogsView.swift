import SwiftUI

struct LogsView: View {
    @StateObject private var vm = LogsViewModel()
    @EnvironmentObject var daemonMonitor: DaemonMonitor

    var body: some View {
        VStack(spacing: 0) {
            // Search and filter bar
            HStack(spacing: 8) {
                // Search field
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12))
                        .foregroundColor(.inTextMuted)
                    TextField("Search run history...", text: $vm.searchQuery)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12.5, design: .monospaced))
                        .foregroundColor(.inTextPrimary)
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 14)
                .background(Color.inDivider)
                .overlay(
                    RoundedRectangle(cornerRadius: INRadius.button)
                        .stroke(Color.inSeparator, lineWidth: 1)
                )
                .cornerRadius(INRadius.button)
                .frame(width: 240)

                Spacer()

                // Filter buttons
                HStack(spacing: 4) {
                    ForEach(LogFilter.allCases, id: \.self) { filter in
                        Button {
                            vm.filter = filter
                            Task { await vm.loadLogs() }
                        } label: {
                            Text(filter.displayName)
                                .font(.system(size: 11.5, weight: .semibold))
                                .foregroundColor(vm.filter == filter ? .inAccentLight : .inTextSubtle)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 6)
                                .background(vm.filter == filter ? Color.inAccentBg : Color.clear)
                                .cornerRadius(INRadius.filter)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(.horizontal, 28)
            .padding(.vertical, 16)

            // Error display
            if let error = vm.error {
                HStack {
                    Image(systemName: "exclamationmark.triangle")
                        .foregroundColor(.inRed)
                    Text(error)
                        .font(.inCaption)
                        .foregroundColor(.inRed)
                }
                .padding(.horizontal, 28)
                .padding(.bottom, 8)
            }

            // Log table
            VStack(spacing: 0) {
                LogTableHeader()

                if vm.isLoading && vm.logs.isEmpty {
                    VStack(spacing: 12) {
                        ProgressView()
                            .scaleEffect(0.8)
                        Text("Fetching run history...")
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextMuted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(INSpacing.p32)
                } else if vm.logs.isEmpty {
                    VStack(spacing: 12) {
                        Image(systemName: "doc.text")
                            .font(.system(size: 40))
                            .foregroundColor(.inTextFaint)
                        Text("No runs yet")
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextMuted)
                        if vm.filter != .all || !vm.searchQuery.isEmpty {
                            Text("Try adjusting your search or filter — or wait for your intern to complete a run")
                                .font(.inCaption)
                                .foregroundColor(.inTextSubtle)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(INSpacing.p32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(vm.logs) { log in
                                LogEntryRow(
                                    log: log,
                                    isExpanded: vm.isExpanded(log.id),
                                    onToggle: { vm.toggleExpanded(log.id) }
                                )
                            }
                        }
                    }
                }
            }
            .background(Color.inSurfaceContainer)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.panel)
                    .stroke(Color.inBorder, lineWidth: INBorder.standard)
            )
            .cornerRadius(INRadius.panel)
            .padding(.horizontal, 28)
            .padding(.bottom, 20)
        }
        .background(Color.inBackground)
        .onAppear {
            vm.setClient(daemonMonitor.client)
            vm.setupSearchDebounce()
            Task { await vm.loadLogs() }
        }
        // Search debounce is handled by setupSearchDebounce()
    }
}
