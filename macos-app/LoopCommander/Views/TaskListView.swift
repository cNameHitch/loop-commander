import SwiftUI

struct TaskListView: View {
    @Binding var selectedTaskId: String?
    let onNewTask: () -> Void
    let onImportCommand: () -> Void

    @EnvironmentObject var taskListVM: TaskListViewModel
    @EnvironmentObject var dashboardVM: DashboardViewModel
    @EnvironmentObject var daemonMonitor: DaemonMonitor

    var body: some View {
        VStack(spacing: 0) {
            // Daemon connection banner
            DaemonBanner(isConnected: daemonMonitor.isConnected) {
                Task { await daemonMonitor.startDaemon() }
            }

            // Metrics bar
            MetricsBarView(
                metrics: dashboardVM.metrics,
                isConnected: daemonMonitor.isConnected,
                taskStatuses: taskListVM.tasks.map(\.status)
            )

            // Toolbar: title + new task button
            HStack {
                Text("Scheduled Tasks")
                    .font(.lcSectionLabel)
                    .foregroundColor(.lcTextMuted)

                Spacer()

                if let error = taskListVM.error {
                    Text(error)
                        .font(.lcCaption)
                        .foregroundColor(.lcRed)
                        .lineLimit(1)
                }

                Button(action: onImportCommand) {
                    HStack(spacing: 5) {
                        Image(systemName: "square.and.arrow.down")
                            .font(.system(size: 11, weight: .semibold))
                        Text("Import")
                            .font(.lcButton)
                    }
                    .foregroundColor(.lcTextPrimary)
                    .padding(.vertical, 7)
                    .padding(.horizontal, 16)
                    .background(Color.lcCodeBackground)
                    .overlay(
                        RoundedRectangle(cornerRadius: LCRadius.button)
                            .stroke(Color.lcBorderInput, lineWidth: 1)
                    )
                    .cornerRadius(LCRadius.button)
                }
                .buttonStyle(.plain)
                .keyboardShortcut("i", modifiers: [.command, .shift])

                Button(action: onNewTask) {
                    HStack(spacing: 5) {
                        Image(systemName: "plus")
                            .font(.system(size: 11, weight: .semibold))
                        Text("New Task")
                            .font(.lcButton)
                    }
                    .foregroundColor(.white)
                    .padding(.vertical, 7)
                    .padding(.horizontal, 16)
                    .background(
                        LinearGradient(
                            colors: [.lcAccent, .lcAccentDeep],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .cornerRadius(LCRadius.button)
                }
                .buttonStyle(.plain)
                .keyboardShortcut("n", modifiers: .command)
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 12)

            // Task table
            VStack(spacing: 0) {
                TaskTableHeader()

                if taskListVM.isLoading && taskListVM.tasks.isEmpty {
                    VStack(spacing: 12) {
                        ProgressView()
                            .scaleEffect(0.8)
                        Text("Loading tasks...")
                            .font(.lcBodyMedium)
                            .foregroundColor(.lcTextMuted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(LCSpacing.p32)
                } else if taskListVM.tasks.isEmpty {
                    VStack(spacing: 12) {
                        Image(systemName: "calendar.badge.plus")
                            .font(.system(size: 40))
                            .foregroundColor(.lcTextFaint)
                        Text("No tasks scheduled")
                            .font(.lcBodyMedium)
                            .foregroundColor(.lcTextMuted)
                        Text("Create your first task to get started")
                            .font(.lcCaption)
                            .foregroundColor(.lcTextSubtle)
                        Button("Create Task") { onNewTask() }
                            .buttonStyle(LCPrimaryButtonStyle())
                            .padding(.top, 8)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(LCSpacing.p32)
                    .accessibilityLabel("No tasks scheduled. Create your first task to get started.")
                } else {
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(taskListVM.tasks) { task in
                                TaskRow(
                                    task: task,
                                    isSelected: selectedTaskId == task.id
                                )
                                .onTapGesture {
                                    selectedTaskId = task.id
                                }
                                .contextMenu {
                                    Button("Run Now") {
                                        Task {
                                            let client = daemonMonitor.client
                                            try? await client.runTaskNow(task.id)
                                        }
                                    }
                                    Divider()
                                    if task.status == .active {
                                        Button("Pause") {
                                            Task { await taskListVM.pauseTask(task.id) }
                                        }
                                    } else if task.status == .paused {
                                        Button("Resume") {
                                            Task { await taskListVM.resumeTask(task.id) }
                                        }
                                    }
                                    Divider()
                                    Button("Delete", role: .destructive) {
                                        Task { await taskListVM.deleteTask(task.id) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            .background(Color.lcSurfaceContainer)
            .overlay(
                RoundedRectangle(cornerRadius: LCRadius.panel)
                    .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
            )
            .cornerRadius(LCRadius.panel)
            .padding(.horizontal, 20)
            .padding(.bottom, 20)
        }
        .background(Color.lcBackground)
        .onAppear {
            Task {
                await taskListVM.loadTasks()
                await dashboardVM.loadMetrics()
                dashboardVM.startRefreshTimer()
            }
        }
        .onDisappear {
            dashboardVM.stopRefreshTimer()
        }
    }
}
