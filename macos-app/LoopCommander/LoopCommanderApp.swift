import SwiftUI

@main
struct LoopCommanderApp: App {
    @StateObject private var daemonMonitor: DaemonMonitor
    @StateObject private var eventStream = EventStream()
    @StateObject private var taskListVM = TaskListViewModel()
    @StateObject private var dashboardVM = DashboardViewModel()
    @StateObject private var notificationManager = NotificationManager()

    init() {
        let client = DaemonClient()
        let monitor = DaemonMonitor(client: client)
        _daemonMonitor = StateObject(wrappedValue: monitor)
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(daemonMonitor)
                .environmentObject(eventStream)
                .environmentObject(taskListVM)
                .environmentObject(dashboardVM)
                .environmentObject(notificationManager)
                .task {
                    // Initialize ViewModels with client
                    taskListVM.setClient(daemonMonitor.client)
                    dashboardVM.setClient(daemonMonitor.client)

                    // Start monitoring daemon connection (idempotent)
                    daemonMonitor.start()

                    // Start event stream (idempotent)
                    eventStream.start()

                    // Setup notifications
                    notificationManager.setup()
                    notificationManager.daemonClient = daemonMonitor.client
                }
        }
        .defaultSize(width: 1200, height: 800)
        .windowResizability(.contentMinSize)
        .windowStyle(.hiddenTitleBar)
        .commands {
            // Replace default "New" with task creation
            CommandGroup(replacing: .newItem) {
                Button("New Task") {
                    NotificationCenter.default.post(name: .editorNewTask, object: nil)
                }
                .keyboardShortcut("n", modifiers: .command)
            }

            // View menu
            CommandMenu("View") {
                Button("Tasks") {
                    NotificationCenter.default.post(name: .switchToTasks, object: nil)
                }
                .keyboardShortcut("1", modifiers: .command)

                Button("Editor") {
                    NotificationCenter.default.post(name: .switchToEditor, object: nil)
                }
                .keyboardShortcut("2", modifiers: .command)

                Button("Logs") {
                    NotificationCenter.default.post(name: .switchToLogs, object: nil)
                }
                .keyboardShortcut("3", modifiers: .command)

                Divider()

                Button("Refresh") {
                    NotificationCenter.default.post(name: .refreshData, object: nil)
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])
            }

            // Task menu (contextual to selected task)
            CommandMenu("Task") {
                Button("Run Now") {
                    // Handled by TaskDetailView keyboard shortcut
                }
                .keyboardShortcut("r", modifiers: .command)

                Button("Dry Run") {
                    // Handled by TaskDetailView keyboard shortcut
                }
                .keyboardShortcut("r", modifiers: [.command, .option])

                Divider()

                Button("Edit...") {
                    // Handled by TaskDetailView keyboard shortcut
                }
                .keyboardShortcut("e", modifiers: .command)

                Button("Pause/Resume") {
                    // Handled by TaskDetailView keyboard shortcut
                }
                .keyboardShortcut("p", modifiers: .command)

                Divider()

                Button("Export...") {
                    // Handled by TaskDetailView
                }
                .keyboardShortcut("e", modifiers: [.command, .shift])

                Button("Import...") {
                    // TODO: Implement import from file
                }
                .keyboardShortcut("i", modifiers: [.command, .shift])

                Divider()

                Button("Delete") {
                    // Handled by TaskDetailView keyboard shortcut
                }
                .keyboardShortcut(.delete, modifiers: .command)
            }
        }

        // Menu bar extra (persistent status item)
        MenuBarExtra("Loop Commander", systemImage: "arrow.triangle.2.circlepath") {
            MenuBarView()
                .environmentObject(daemonMonitor)
                .environmentObject(dashboardVM)
        }
        .menuBarExtraStyle(.window)
    }
}

// MARK: - Menu Bar View

struct MenuBarView: View {
    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @EnvironmentObject var dashboardVM: DashboardViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Daemon status
            HStack {
                Circle()
                    .fill(daemonMonitor.isConnected ? Color.lcGreen : Color.lcRed)
                    .frame(width: 8, height: 8)
                Text(daemonMonitor.isConnected ? "Daemon Running" : "Daemon Offline")
                    .font(.system(size: 12, weight: .medium))
            }
            .accessibilityLabel(daemonMonitor.isConnected ? "Connected to daemon" : "Disconnected from daemon")

            Divider()

            // Quick stats
            Text("\(dashboardVM.metrics.activeTasks) active tasks")
                .font(.system(size: 11))
            Text("Success rate: \(Int(dashboardVM.metrics.overallSuccessRate))%")
                .font(.system(size: 11))
            Text("Total spend: \(formatCost(dashboardVM.metrics.totalSpend))")
                .font(.system(size: 11))

            Divider()

            // Quick actions
            Button("Open Dashboard") {
                NSApp.activate(ignoringOtherApps: true)
                if let window = NSApp.windows.first(where: { $0.canBecomeMain }) {
                    window.makeKeyAndOrderFront(nil)
                } else {
                    // If window was closed, open a new one
                    NSApp.sendAction(Selector(("newWindowForTab:")), to: nil, from: nil)
                }
            }

            Button("New Task...") {
                NSApp.activate(ignoringOtherApps: true)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    NotificationCenter.default.post(name: .editorNewTask, object: nil)
                }
            }

            Button("Refresh Data") {
                Task { await dashboardVM.loadMetrics() }
            }

            if !daemonMonitor.isConnected {
                Button("Start Daemon") {
                    Task { await daemonMonitor.startDaemon() }
                }
            }

            Divider()

            Button("Quit Loop Commander") {
                NSApp.terminate(nil)
            }
        }
        .padding(8)
        .frame(width: 220)
        .onAppear {
            Task { await dashboardVM.loadMetrics() }
        }
    }
}
