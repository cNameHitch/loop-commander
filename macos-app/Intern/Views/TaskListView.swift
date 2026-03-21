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
                Text("Assigned Tasks")
                    .font(.inSectionLabel)
                    .foregroundColor(.inTextMuted)

                Spacer()

                if let error = taskListVM.error {
                    Text(error)
                        .font(.inCaption)
                        .foregroundColor(.inRed)
                        .lineLimit(1)
                }

                Button(action: onImportCommand) {
                    HStack(spacing: 5) {
                        Image(systemName: "square.and.arrow.down")
                            .font(.system(size: 11, weight: .semibold))
                        Text("Import")
                            .font(.inButton)
                    }
                    .foregroundColor(.inTextPrimary)
                    .padding(.vertical, 7)
                    .padding(.horizontal, 16)
                    .background(Color.inCodeBackground)
                    .overlay(
                        RoundedRectangle(cornerRadius: INRadius.button)
                            .stroke(Color.inBorderInput, lineWidth: 1)
                    )
                    .cornerRadius(INRadius.button)
                }
                .buttonStyle(.plain)
                .keyboardShortcut("i", modifiers: [.command, .shift])

                Button(action: onNewTask) {
                    HStack(spacing: 5) {
                        Image(systemName: "plus")
                            .font(.system(size: 11, weight: .semibold))
                        Text("Assign Task")
                            .font(.inButton)
                    }
                    .foregroundColor(.white)
                    .padding(.vertical, 7)
                    .padding(.horizontal, 16)
                    .background(
                        LinearGradient(
                            colors: [.inAccent, .inAccentDeep],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .cornerRadius(INRadius.button)
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
                        Text("Checking in with your intern...")
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextMuted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(INSpacing.p32)
                } else if taskListVM.tasks.isEmpty {
                    VStack(spacing: 12) {
                        Image(systemName: "calendar.badge.plus")
                            .font(.system(size: 40))
                            .foregroundColor(.inTextFaint)
                        Text("Your intern is waiting for work")
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextMuted)
                        Text("Assign a task and they'll get right to it")
                            .font(.inCaption)
                            .foregroundColor(.inTextSubtle)
                        Button("Assign First Task") { onNewTask() }
                            .buttonStyle(INPrimaryButtonStyle())
                            .padding(.top, 8)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(INSpacing.p32)
                    .accessibilityLabel("No tasks assigned. Assign a task and your intern will get right to it.")
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
            .background(Color.inSurfaceContainer)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.panel)
                    .stroke(Color.inBorder, lineWidth: INBorder.standard)
            )
            .cornerRadius(INRadius.panel)
            .padding(.horizontal, 20)
            .padding(.bottom, 20)
        }
        .background(Color.inBackground)
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
