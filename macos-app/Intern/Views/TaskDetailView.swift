import SwiftUI

struct TaskDetailView: View {
    let taskId: String
    let onEdit: (INTask) -> Void
    let onDelete: () -> Void

    @StateObject private var vm = TaskDetailViewModel()
    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @State private var expandedLogIds: Set<Int> = []
    @State private var showDeleteConfirmation = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if vm.isLoading && vm.task == nil {
                    VStack {
                        ProgressView()
                        Text("Loading task...")
                            .font(.inBodyMedium)
                            .foregroundColor(.inTextMuted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(INSpacing.p32)
                } else if let task = vm.task {
                    // Action bar
                    actionBar(task: task)

                    // Task info card
                    taskInfoCard(task: task)

                    // Execution history
                    executionHistory
                } else if let error = vm.error {
                    VStack(spacing: 12) {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: 40))
                            .foregroundColor(.inRed)
                        Text("Error loading task")
                            .font(.inBodyBold)
                            .foregroundColor(.inTextPrimary)
                        Text(error)
                            .font(.inCaption)
                            .foregroundColor(.inTextMuted)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(INSpacing.p32)
                }
            }
            .padding(INSpacing.p20)
        }
        .background(Color.inBackground)
        .onAppear {
            vm.setClient(daemonMonitor.client)
            Task { await vm.loadTask(taskId) }
        }
        .onChange(of: taskId) { newId in
            Task { await vm.loadTask(newId) }
        }
        .overlay {
            if vm.showDryRun, let result = vm.dryRunResult {
                Color.inOverlay
                    .ignoresSafeArea()
                    .onTapGesture { vm.showDryRun = false }
                    .transition(.opacity)

                dryRunSheet(result: result)
                    .shadow(color: .black.opacity(0.4), radius: 24, y: 8)
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: vm.showDryRun)
        .alert("Delete Task?", isPresented: $showDeleteConfirmation) {
            Button("Cancel", role: .cancel) {}
            Button("Delete", role: .destructive) {
                Task {
                    if await vm.deleteTask() {
                        onDelete()
                    }
                }
            }
        } message: {
            Text("This will permanently remove the task and its launchd schedule. Execution logs will be preserved.")
        }
    }

    // MARK: - Action Bar

    @ViewBuilder
    private func actionBar(task: INTask) -> some View {
        HStack(spacing: 10) {
            // Run Now / Stop
            if task.status == .running {
                Button {
                    Task { await vm.stopTask() }
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: "stop.fill")
                            .font(.system(size: 10))
                        Text("Stop")
                    }
                }
                .buttonStyle(INToolbarButtonStyle(foreground: .inRed))
                .keyboardShortcut("r", modifiers: .command)
                .help("Stop the running task (Cmd+R)")
            } else {
                Button {
                    Task { await vm.runNow() }
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: "play.fill")
                            .font(.system(size: 10))
                        Text("Run Now")
                    }
                }
                .buttonStyle(INToolbarButtonStyle(foreground: .inGreen))
                .keyboardShortcut("r", modifiers: .command)
                .help("Execute this task immediately (Cmd+R)")
            }

            // Edit
            Button {
                onEdit(task)
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: "pencil")
                        .font(.system(size: 10))
                    Text("Edit")
                }
            }
            .buttonStyle(INToolbarButtonStyle())
            .keyboardShortcut("e", modifiers: .command)
            .help("Edit task configuration (Cmd+E)")

            Spacer()

            // Pause/Resume
            Button {
                Task {
                    if task.status == .active {
                        await vm.pauseTask()
                    } else {
                        await vm.resumeTask()
                    }
                }
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: task.status == .active ? "pause.fill" : "play.fill")
                        .font(.system(size: 10))
                    Text(task.status == .active ? "Pause" : "Resume")
                }
            }
            .buttonStyle(INToolbarButtonStyle(
                foreground: task.status == .active ? .inAmber : .inGreen
            ))
            .keyboardShortcut("p", modifiers: .command)
            .help(task.status == .active ? "Pause this task" : "Resume this task")

            // Delete
            Button {
                showDeleteConfirmation = true
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: "trash")
                        .font(.system(size: 10))
                    Text("Delete")
                }
            }
            .buttonStyle(INDangerButtonStyle())
            .keyboardShortcut(.delete, modifiers: .command)
            .help("Delete this task permanently")
        }
        .padding(.bottom, 20)
        .transition(.inFadeSlide)
    }

    // MARK: - Task Info Card

    @ViewBuilder
    private func taskInfoCard(task: INTask) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            // Title row
            HStack(spacing: 12) {
                Text(task.name)
                    .font(.inHeadingLarge)
                    .foregroundColor(.inTextPrimary)
                StatusBadge(status: task.status)
            }
            .padding(.bottom, 16)

            // Two-column content
            HStack(alignment: .top, spacing: 20) {
                // Left: Command preview
                VStack(alignment: .leading, spacing: 6) {
                    Text("COMMAND")
                        .font(.inFieldLabel)
                        .foregroundColor(.inTextSubtle)
                        .textCase(.uppercase)
                        .tracking(0.5)

                    Text(truncateToLines(task.command, maxLines: 25))
                        .font(.inCodePreview)
                        .foregroundColor(.inAccentLight)
                        .lineSpacing(5)
                        .lineLimit(25)
                        .padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color.inCodeBackground)
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(Color.inBorder, lineWidth: 1)
                        )
                        .cornerRadius(6)
                        .textSelection(.enabled)
                }
                .frame(maxWidth: .infinity)

                // Right: Metadata grid
                VStack(spacing: 12) {
                    metadataRow(label: "Schedule", value: task.scheduleHuman)
                    metadataRow(label: "Cron", value: task.schedule.cronExpression ?? "\u{2014}")
                    metadataRow(label: "Working Dir", value: abbreviatePath(task.workingDir))
                    metadataRow(label: "Total Spent", value: formatCost(task.totalCost))
                    metadataRow(label: "Created", value: relativeTime(task.createdAt))
                    metadataRow(label: "Last Run", value: task.lastRun.map { relativeTime($0) } ?? "\u{2014}")
                }
                .frame(maxWidth: .infinity)
            }

            // Tags
            if !task.tags.isEmpty {
                HStack(spacing: 4) {
                    ForEach(task.tags, id: \.self) { tag in
                        TagChip(text: tag)
                    }
                }
                .padding(.top, 12)
            }
        }
        .padding(24)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.panel)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.panel)
        .padding(.bottom, 20)
    }

    private func truncateToLines(_ text: String, maxLines: Int) -> String {
        let lines = text.components(separatedBy: .newlines)
        if lines.count <= maxLines { return text }
        return lines.prefix(maxLines).joined(separator: "\n") + "\n..."
    }

    private func metadataRow(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(.inFieldLabel)
                .foregroundColor(.inTextFaint)
                .textCase(.uppercase)
                .tracking(0.5)
            Text(value)
                .font(.inFieldValue)
                .foregroundColor(.inTextSecondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Execution History

    @ViewBuilder
    private var executionHistory: some View {
        VStack(spacing: 0) {
            // Section header
            HStack {
                Text("Execution History (\(vm.logs.count) runs)")
                    .font(.inSectionLabel)
                    .foregroundColor(.inTextMuted)
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .overlay(alignment: .bottom) {
                Rectangle().fill(Color.inBorder).frame(height: 1)
            }

            if vm.logs.isEmpty {
                Text("No executions yet")
                    .font(.system(size: 13))
                    .foregroundColor(.inTextDimmest)
                    .padding(INSpacing.p32)
                    .frame(maxWidth: .infinity)
                    .accessibilityLabel("No execution history available for this task")
            } else {
                LogTableHeader()

                LazyVStack(spacing: 0) {
                    ForEach(vm.logs) { log in
                        LogEntryRow(
                            log: log,
                            isExpanded: expandedLogIds.contains(log.id),
                            onToggle: {
                                if expandedLogIds.contains(log.id) {
                                    expandedLogIds.remove(log.id)
                                } else {
                                    expandedLogIds.insert(log.id)
                                }
                            }
                        )
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
    }

    // MARK: - Dry Run Sheet

    @ViewBuilder
    private func dryRunSheet(result: DryRunResult) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack {
                Text("Dry Run Result")
                    .font(.inHeading)
                    .foregroundColor(.inTextPrimary)
                Spacer()
                Button("Close") { vm.showDryRun = false }
                    .buttonStyle(INSecondaryButtonStyle())
            }

            if result.wouldBeSkipped {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.inAmber)
                    Text("Task would be skipped: \(result.skipReason ?? "Unknown reason")")
                        .font(.inBodyMedium)
                        .foregroundColor(.inAmber)
                }
                .padding(12)
                .background(Color.inAmberBg)
                .cornerRadius(INRadius.button)
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("RESOLVED COMMAND")
                    .font(.inFieldLabel)
                    .foregroundColor(.inTextSubtle)
                    .textCase(.uppercase)
                    .tracking(0.5)
                Text(result.resolvedCommand.joined(separator: " "))
                    .font(.inCode)
                    .foregroundColor(.inAccentLight)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.inCodeBackground)
                    .cornerRadius(6)
                    .textSelection(.enabled)
            }

            HStack(spacing: 20) {
                metadataRow(label: "Working Dir", value: abbreviatePath(result.workingDir))
                metadataRow(label: "Timeout", value: formatDuration(result.timeoutSecs))
            }

            HStack(spacing: 20) {
                metadataRow(label: "Daily Spend", value: formatCost(result.dailySpendSoFar))
            }

            if !result.envVars.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("ENVIRONMENT VARIABLES")
                        .font(.inFieldLabel)
                        .foregroundColor(.inTextSubtle)
                        .textCase(.uppercase)
                        .tracking(0.5)
                    ForEach(Array(result.envVars.keys.sorted()), id: \.self) { key in
                        Text("\(key)=\(result.envVars[key] ?? "")")
                            .font(.inCode)
                            .foregroundColor(.inTextSecondary)
                    }
                }
            }
        }
        .padding(32)
        .frame(width: 560)
        .background(Color.inSurface)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.modal)
                .stroke(Color.inSeparator, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.modal)
    }
}
