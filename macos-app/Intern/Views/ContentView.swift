import SwiftUI

/// Notification names for cross-view communication
extension Notification.Name {
    static let newTask = Notification.Name("com.intern.newTask")
    static let refreshData = Notification.Name("com.intern.refreshData")
    static let switchToTasks = Notification.Name("com.intern.switchToTasks")
    static let switchToLogs = Notification.Name("com.intern.switchToLogs")
    static let editorNewTask = Notification.Name("com.intern.editorNewTask")
    static let editorOpenTask = Notification.Name("com.intern.editorOpenTask")
    static let editorOpenImport = Notification.Name("com.intern.editorOpenImport")
    static let switchToEditor = Notification.Name("com.intern.switchToEditor")
}

struct ContentView: View {
    @State private var selectedSidebar: SidebarItem? = .tasks
    @State private var selectedTaskId: String? = nil
    @State private var showingImport = false
    @StateObject private var editorVM = EditorViewModel()
    @State private var showingDirtyAlert: Bool = false
    @State private var pendingNavigation: SidebarItem? = nil

    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @EnvironmentObject var taskListVM: TaskListViewModel
    @EnvironmentObject var dashboardVM: DashboardViewModel
    @EnvironmentObject var eventStream: EventStream
    @EnvironmentObject var notificationManager: NotificationManager

    var body: some View {
        HSplitView {
            // Sidebar
            SidebarView(
                selection: guardedSelection,
                activeCount: taskListVM.tasks.filter { $0.status == .active || $0.status == .running }.count,
                editorIsDirty: editorVM.isDirty
            )
            .frame(minWidth: 205, idealWidth: 220, maxWidth: 260)

            // Main content — ZStack keeps both tabs alive so split position
            // and scroll state are preserved when switching tabs.
            ZStack {
                tasksLayout
                    .opacity(selectedSidebar == .tasks ? 1 : 0)
                    .allowsHitTesting(selectedSidebar == .tasks)

                EditorView(vm: editorVM)
                    .opacity(selectedSidebar == .editor ? 1 : 0)
                    .allowsHitTesting(selectedSidebar == .editor)

                LogsView()
                    .opacity(selectedSidebar == .logs ? 1 : 0)
                    .allowsHitTesting(selectedSidebar == .logs)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color.inBackground)
            .animation(.easeInOut(duration: 0.15), value: selectedSidebar)
        }
        .overlay {
            if showingImport {
                Color.inOverlay
                    .ignoresSafeArea()
                    .onTapGesture { showingImport = false }
                    .transition(.opacity)

                CommandImportView(
                    onImport: { command in
                        showingImport = false
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                            NotificationCenter.default.post(
                                name: .editorOpenImport,
                                object: nil,
                                userInfo: ["command": command]
                            )
                        }
                    },
                    onDismiss: { showingImport = false }
                )
                .inModalShadow()
                .transition(.scale(scale: 0.95).combined(with: .opacity))
                .onExitCommand { showingImport = false }
            }
        }
        .animation(.easeInOut(duration: 0.2), value: showingImport)
        .alert("Unsaved task changes", isPresented: $showingDirtyAlert) {
            Button("Save") {
                Task {
                    let saved = await editorVM.save()
                    if saved, let dest = pendingNavigation {
                        selectedSidebar = dest
                        pendingNavigation = nil
                    }
                }
            }
            Button("Discard", role: .destructive) {
                editorVM.discard()
                if let dest = pendingNavigation {
                    selectedSidebar = dest
                    pendingNavigation = nil
                }
            }
            Button("Cancel", role: .cancel) {
                pendingNavigation = nil
            }
        } message: {
            Text("Save your changes before leaving, or discard them.")
        }
        .frame(minWidth: 900, minHeight: 600)
        .onAppear {
            setupEventHandlers()
        }
        .onReceive(NotificationCenter.default.publisher(for: .refreshData)) { _ in
            Task {
                await taskListVM.loadTasks()
                await dashboardVM.loadMetrics()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchToTasks)) { _ in
            selectedSidebar = .tasks
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchToLogs)) { _ in
            selectedSidebar = .logs
        }
        .onReceive(NotificationCenter.default.publisher(for: .editorNewTask)) { _ in
            editorVM.startNewTask()
            selectedSidebar = .editor
        }
        .onReceive(NotificationCenter.default.publisher(for: .editorOpenTask)) { notification in
            if let task = notification.userInfo?["task"] as? INTask {
                editorVM.loadTask(task)
                selectedSidebar = .editor
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .editorOpenImport)) { notification in
            if let command = notification.userInfo?["command"] as? ClaudeCommand {
                editorVM.loadFromImportedCommand(command)
                selectedSidebar = .editor
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchToEditor)) { _ in
            selectedSidebar = .editor
        }
    }

    // MARK: - Guarded Sidebar Selection

    private var guardedSelection: Binding<SidebarItem?> {
        Binding(
            get: { selectedSidebar },
            set: { newValue in
                if selectedSidebar == .editor && editorVM.isDirty && newValue != .editor {
                    pendingNavigation = newValue
                    showingDirtyAlert = true
                } else {
                    selectedSidebar = newValue
                }
            }
        )
    }

    // MARK: - Tasks Layout (fixed 50/50 split)

    @ViewBuilder
    private var tasksLayout: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                TaskListView(
                    selectedTaskId: $selectedTaskId,
                    onNewTask: {
                        NotificationCenter.default.post(name: .editorNewTask, object: nil)
                    },
                    onImportCommand: { showingImport = true }
                )
                .frame(width: geo.size.width / 2)
                .clipped()

                Rectangle()
                    .fill(Color.inSeparator)
                    .frame(width: 1)

                Group {
                    if let taskId = selectedTaskId {
                        TaskDetailView(
                            taskId: taskId,
                            onEdit: { task in
                                NotificationCenter.default.post(
                                    name: .editorOpenTask,
                                    object: nil,
                                    userInfo: ["task": task]
                                )
                            },
                            onDelete: {
                                selectedTaskId = nil
                                Task { await taskListVM.loadTasks() }
                            }
                        )
                    } else {
                        VStack(spacing: 12) {
                            Image(systemName: "sidebar.right")
                                .font(.system(size: 36))
                                .foregroundColor(.inTextFaint)
                            Text("Pick a task to review")
                                .font(.inBodyMedium)
                                .foregroundColor(.inTextMuted)
                            Text("Select one from the list to see how your intern is doing")
                                .font(.inCaption)
                                .foregroundColor(.inTextSubtle)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(Color.inBackground)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func setupEventHandlers() {
        eventStream.onAnyEvent = { [weak taskListVM, weak dashboardVM] in
            Task { @MainActor in
                await taskListVM?.loadTasks()
                await dashboardVM?.loadMetrics()
            }
        }

        let nm = notificationManager
        eventStream.onTaskCompleted = { taskId, taskName, durationSecs, costUsd in
            Task { @MainActor in
                nm.sendTaskSuccess(
                    taskId: taskId,
                    taskName: taskName,
                    durationSecs: durationSecs,
                    costUsd: costUsd
                )
            }
        }

        eventStream.onTaskFailed = { taskId, taskName, exitCode, summary in
            Task { @MainActor in
                if summary.lowercased().contains("timeout") {
                    nm.sendTaskTimeout(
                        taskId: taskId,
                        taskName: taskName,
                        summary: summary
                    )
                } else {
                    nm.sendTaskFailure(
                        taskId: taskId,
                        taskName: taskName,
                        exitCode: exitCode,
                        summary: summary
                    )
                }
            }
        }
    }
}
