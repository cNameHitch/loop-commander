import SwiftUI

@main
struct InternApp: App {
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
        WindowGroup(id: "main") {
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

                    // Auto-launch daemon if not connected after a brief wait
                    Task {
                        try? await Task.sleep(nanoseconds: 2_000_000_000)
                        if !daemonMonitor.isConnected {
                            await daemonMonitor.startDaemon()
                        }
                    }

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
                Button("Assign New Task") {
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

                Button("Run History") {
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
        MenuBarExtra("Intern", systemImage: "person.badge.clock.fill") {
            MenuBarView()
                .environmentObject(daemonMonitor)
                .environmentObject(dashboardVM)
        }
        .menuBarExtraStyle(.menu)
    }
}

// MARK: - Menu Bar View

struct MenuBarView: View {
    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @EnvironmentObject var dashboardVM: DashboardViewModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        // Daemon status
        Text(daemonMonitor.isConnected ? "● Intern is active" : "○ Intern is offline")

        // Quick stats
        Text("\(dashboardVM.metrics.activeTasks) tasks running")
        Text("Success rate: \(Int(dashboardVM.metrics.overallSuccessRate))%")
        Text("Total spend: \(formatCost(dashboardVM.metrics.totalSpend))")

        Divider()

        // Quick actions
        Button("Open Intern") {
            bringAppToFront()
        }

        Button("Assign New Task...") {
            bringAppToFront()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                NotificationCenter.default.post(name: .editorNewTask, object: nil)
            }
        }

        Button("Refresh") {
            Task { await dashboardVM.loadMetrics() }
            NotificationCenter.default.post(name: .refreshData, object: nil)
        }

        if !daemonMonitor.isConnected {
            Button("Start Intern") {
                Task { await daemonMonitor.startDaemon() }
            }
        }

        Divider()

        Button("Quit Intern") {
            NSApp.terminate(nil)
        }
    }

    private func bringAppToFront() {
        // Use openWindow to ensure the WindowGroup window exists
        openWindow(id: "main")

        // Activate the application
        if #available(macOS 14.0, *) {
            NSApp.activate()
        } else {
            NSApp.activate(ignoringOtherApps: true)
        }

        // Make all normal windows key and front
        DispatchQueue.main.async {
            for window in NSApp.windows where window.canBecomeMain {
                window.makeKeyAndOrderFront(nil)
                break
            }
        }
    }
}
