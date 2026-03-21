import SwiftUI

struct EditorView: View {
    @ObservedObject var vm: EditorViewModel
    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @EnvironmentObject var taskListVM: TaskListViewModel
    @State private var tagInput: String = ""
    @State private var discoveredCommands: [ClaudeCommand] = []
    @State private var skillSearchText: String = ""
    @State private var showPreview: Bool = false
    @State private var skillsCurrentPage: Int = 0
    private let skillsPerPage: Int = 6

    private var filteredCommands: [ClaudeCommand] {
        if skillSearchText.isEmpty { return discoveredCommands }
        let query = skillSearchText.lowercased()
        return discoveredCommands.filter {
            $0.name.lowercased().contains(query) ||
            $0.description.lowercased().contains(query) ||
            $0.projectName.lowercased().contains(query)
        }
    }

    private var skillsTotalPages: Int {
        max(1, Int(ceil(Double(filteredCommands.count) / Double(skillsPerPage))))
    }

    private var paginatedCommands: [ClaudeCommand] {
        let start = skillsCurrentPage * skillsPerPage
        let end = min(start + skillsPerPage, filteredCommands.count)
        guard start < filteredCommands.count else { return [] }
        return Array(filteredCommands[start..<end])
    }

    var body: some View {
        Group {
            switch vm.editorState {
            case .empty:
                emptyState
            case .creating, .editing:
                editorContent
            }
        }
        .onChange(of: vm.editorState) { newState in
            showPreview = false
            if newState == .empty {
                discoveredCommands = CommandScanner.scan()
                Task { await taskListVM.loadTasks() }
            }
        }
    }

    // MARK: - Empty State

    private var emptyState: some View {
        VStack(spacing: 0) {
            // Header bar
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Task Editor")
                        .font(.inHeadingLarge)
                        .foregroundColor(.inTextPrimary)
                    Text("Write the instructions your intern will follow")
                        .font(.inCaption)
                        .foregroundColor(.inTextSubtle)
                }
                Spacer()
                Button("+ Assign New Task") {
                    vm.startNewTask()
                }
                .buttonStyle(INPrimaryButtonStyle())
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            .background(Color.inSurface)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(Color.inSeparator)
                    .frame(height: 1)
            }

            // Content
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 16) {
                    // Tasks section
                    tasksListSection
                    // Skills section
                    skillsListSection
                }
                .padding(20)
            }
            .background(Color.inBackground)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear {
            discoveredCommands = CommandScanner.scan()
            Task { await taskListVM.loadTasks() }
        }
    }

    // MARK: - Tasks List Section

    private var tasksListSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("TASKS")
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .tracking(0.5)

            if taskListVM.tasks.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "tray")
                        .foregroundColor(.inTextFaint)
                    Text("No tasks yet — assign one and your intern will get started.")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.inSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: INRadius.panel)
                        .stroke(Color.inBorder, lineWidth: INBorder.standard)
                )
                .cornerRadius(INRadius.panel)
            } else {
                VStack(spacing: 4) {
                    ForEach(taskListVM.tasks) { task in
                        taskCard(task)
                    }
                }
            }
        }
    }

    // MARK: - Task Card

    private func taskCard(_ task: INTask) -> some View {
        HStack(spacing: 12) {
            // Status dot
            Circle()
                .fill(statusColor(for: task.status))
                .frame(width: 8, height: 8)

            // Task info
            VStack(alignment: .leading, spacing: 2) {
                Text(task.name)
                    .font(.inBodyMedium)
                    .foregroundColor(.inTextPrimary)
                    .lineLimit(1)
                Text(task.scheduleHuman)
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
            }

            Spacer()

            // Status label
            Text(task.status.rawValue.capitalized)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(statusColor(for: task.status))
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(statusColor(for: task.status).opacity(0.15))
                .cornerRadius(INRadius.badge)

            // Optimize button (existing tasks with history)
            Button("Optimize") {
                NotificationCenter.default.post(
                    name: .editorOpenTask,
                    object: nil,
                    userInfo: ["task": task]
                )
            }
            .buttonStyle(INToolbarButtonStyle())

            // Edit button
            Button("Edit") {
                NotificationCenter.default.post(
                    name: .editorOpenTask,
                    object: nil,
                    userInfo: ["task": task]
                )
            }
            .buttonStyle(INToolbarButtonStyle())
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.panel)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.panel)
    }

    // MARK: - Status Color

    private func statusColor(for status: TaskStatus) -> Color {
        switch status {
        case .active:
            return .inGreen
        case .paused:
            return .inAmber
        case .error:
            return .inRed
        case .running:
            return .inAccent
        case .disabled:
            return .inTextFaint
        }
    }

    // MARK: - Skills List Section

    private var skillsListSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("SKILLS & COMMANDS")
                    .font(.inLabel)
                    .foregroundColor(.inTextMuted)
                    .tracking(0.5)
                Spacer()
                if !discoveredCommands.isEmpty {
                    let rangeStart = filteredCommands.isEmpty ? 0 : skillsCurrentPage * skillsPerPage + 1
                    let rangeEnd = min((skillsCurrentPage + 1) * skillsPerPage, filteredCommands.count)
                    Text(
                        filteredCommands.isEmpty
                            ? "0 of \(discoveredCommands.count)"
                            : "Showing \(rangeStart)-\(rangeEnd) of \(filteredCommands.count)"
                    )
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
                }
            }

            if !discoveredCommands.isEmpty {
                INTextField(
                    text: $skillSearchText,
                    placeholder: "Filter skills or commands..."
                )
                .onChange(of: skillSearchText) { _ in
                    skillsCurrentPage = 0
                }
            }

            if discoveredCommands.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.inTextFaint)
                    Text("No Claude Code commands found. Add .md files to .claude/commands/ in your projects.")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.inSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: INRadius.panel)
                        .stroke(Color.inBorder, lineWidth: INBorder.standard)
                )
                .cornerRadius(INRadius.panel)
            } else if filteredCommands.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.inTextFaint)
                    Text("No skills or commands match \"\(skillSearchText)\"")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.inSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: INRadius.panel)
                        .stroke(Color.inBorder, lineWidth: INBorder.standard)
                )
                .cornerRadius(INRadius.panel)
            } else {
                VStack(spacing: 4) {
                    ForEach(paginatedCommands) { command in
                        skillCard(command)
                    }
                }

                if skillsTotalPages > 1 {
                    skillsPaginationControls
                }
            }
        }
    }

    // MARK: - Skills Pagination Controls

    private var skillsPaginationControls: some View {
        HStack(spacing: 12) {
            Button {
                withAnimation(.inQuick) {
                    skillsCurrentPage = max(0, skillsCurrentPage - 1)
                }
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 11, weight: .medium))
                    Text("Previous")
                }
            }
            .buttonStyle(INToolbarButtonStyle())
            .disabled(skillsCurrentPage == 0)
            .opacity(skillsCurrentPage == 0 ? 0.4 : 1.0)

            Spacer()

            Text("Page \(skillsCurrentPage + 1) of \(skillsTotalPages)")
                .font(.inCaption)
                .foregroundColor(.inTextMuted)

            Spacer()

            Button {
                withAnimation(.inQuick) {
                    skillsCurrentPage = min(skillsTotalPages - 1, skillsCurrentPage + 1)
                }
            } label: {
                HStack(spacing: 4) {
                    Text("Next")
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .medium))
                }
            }
            .buttonStyle(INToolbarButtonStyle())
            .disabled(skillsCurrentPage >= skillsTotalPages - 1)
            .opacity(skillsCurrentPage >= skillsTotalPages - 1 ? 0.4 : 1.0)
        }
        .padding(.top, 4)
    }

    // MARK: - Skill Card

    private func skillCard(_ command: ClaudeCommand) -> some View {
        HStack(spacing: 12) {
            // Skill icon
            Image(systemName: "terminal")
                .font(.system(size: 14))
                .foregroundColor(.inAccentLight)
                .frame(width: 24)

            // Skill info
            VStack(alignment: .leading, spacing: 2) {
                Text("/\(command.name)")
                    .font(.inBodyMedium)
                    .foregroundColor(.inTextPrimary)
                    .lineLimit(1)
                if !command.description.isEmpty {
                    Text(command.description)
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                        .lineLimit(2)
                }
            }

            Spacer()

            // Project tag
            Text(command.projectName)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(.inAccentLight)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(Color.inTagBg)
                .cornerRadius(INRadius.badge)

            // Open button
            Button("Open") {
                NotificationCenter.default.post(
                    name: .editorOpenImport,
                    object: nil,
                    userInfo: ["command": command]
                )
            }
            .buttonStyle(INToolbarButtonStyle())
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.panel)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.panel)
    }

    // MARK: - Editor Content

    private var editorContent: some View {
        VStack(spacing: 0) {
            editorTopBar
            unsavedChangesBanner
            editorPanes

            if !vm.validationErrors.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(vm.validationErrors, id: \.self) { err in
                        HStack(spacing: 6) {
                            Image(systemName: "exclamationmark.circle.fill")
                                .font(.system(size: 11))
                                .foregroundColor(.inRed)
                            Text(err)
                                .font(.inCaption)
                                .foregroundColor(.inRed)
                        }
                    }
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 8)
                .background(Color.inBackground)
            }

            if let error = vm.error {
                HStack(spacing: 6) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.inRed)
                    Text(error)
                        .font(.inCaption)
                        .foregroundColor(.inRed)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 8)
                .background(Color.inBackground)
            }
        }
        .animation(.inQuick, value: vm.isDirty)
        .onAppear {
            vm.setClient(daemonMonitor.client)
        }
        .alert("Discard changes?", isPresented: $vm.showDiscardAlert) {
            Button("Discard", role: .destructive) { vm.discard() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Your unsaved edits will be lost.")
        }
    }

    // MARK: - Editor Top Bar

    @ViewBuilder
    private var editorTopBar: some View {
        HStack(spacing: 8) {
            TextField("Untitled Task", text: $vm.draft.name)
                .font(.inHeadingLarge)
                .textFieldStyle(.plain)
                .foregroundColor(.inTextPrimary)
                .accessibilityLabel("Task name")

            Spacer()

            Button("Discard") {
                if vm.isDirty {
                    vm.confirmDiscard()
                } else {
                    vm.editorState = .empty
                }
            }
            .buttonStyle(INSecondaryButtonStyle())
            .keyboardShortcut(.escape, modifiers: [])

            Button(vm.isCreating ? "Create Task" : "Save Changes") {
                Task { await vm.save() }
            }
            .buttonStyle(INPrimaryButtonStyle())
            .disabled(vm.isSaving)
            .keyboardShortcut("s", modifiers: .command)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
        .background(Color.inSurface)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.inSeparator)
                .frame(height: 1)
        }
    }

    // MARK: - Unsaved Changes Banner

    @ViewBuilder
    private var unsavedChangesBanner: some View {
        if vm.isDirty {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 11))
                    .foregroundColor(.inAmber)
                Text("Unsaved changes -- you have edits in progress.")
                    .font(.inBodyMedium)
                    .foregroundColor(.inAmber)
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(Color.inAmberBg)
            .overlay(alignment: .top) {
                Rectangle()
                    .fill(Color.inAmber.opacity(0.2))
                    .frame(height: 1)
            }
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(Color.inSeparator)
                    .frame(height: 1)
            }
            .transition(.inFadeSlide)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isStaticText)
        }
    }

    // MARK: - Editor Panes (60/40 split)

    private var editorPanes: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                promptEditorPane
                    .frame(width: geo.size.width * 0.6)

                Rectangle()
                    .fill(Color.inSeparator)
                    .frame(width: 1)

                settingsPane
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    // MARK: - Prompt Editor Pane

    private var promptEditorPane: some View {
        VStack(spacing: 0) {
            HStack {
                Text("PROMPT / COMMAND")
                    .font(.inLabel)
                    .foregroundColor(.inTextMuted)
                    .tracking(0.5)
                    .textCase(.uppercase)
                Spacer()
                previewToggle
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)

            if showPreview {
                MarkdownPreviewView(text: vm.draft.command)
                    .frame(maxHeight: .infinity)
            } else {
                INTextEditor(
                    text: $vm.draft.command,
                    placeholder: "claude -p 'Your prompt here...'"
                )
                .frame(maxHeight: .infinity)
            }

            Rectangle()
                .fill(Color.inBorder)
                .frame(height: 1)
            HStack {
                Text("\(vm.draft.command.count) characters")
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
                Spacer()
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
        }
        .background(Color.inBackground)
    }

    // MARK: - Preview Toggle

    private var previewToggle: some View {
        HStack(spacing: 0) {
            toggleSegment(label: "Code", isActive: !showPreview) {
                showPreview = false
            }
            toggleSegment(label: "Preview", isActive: showPreview) {
                showPreview = true
            }
        }
        .background(Color.inCodeBackground)
        .cornerRadius(INRadius.filter)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.filter)
                .stroke(Color.inBorderInput, lineWidth: 1)
        )
    }

    private func toggleSegment(
        label: String,
        isActive: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(label)
                .font(.inButtonSmall)
                .foregroundColor(isActive ? .inAccentLight : .inTextMuted)
                .padding(.vertical, 5)
                .padding(.horizontal, 10)
                .background(isActive ? Color.inAccentBg : Color.clear)
                .cornerRadius(INRadius.filter)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Settings Pane

    private var settingsPane: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(spacing: 4) {
                // AI Prompt Generator (new tasks only)
                if vm.isCreating {
                    PromptGeneratorPanel(
                        vm: vm.promptGeneratorVM,
                        draft: $vm.draft,
                        workingDir: vm.draft.workingDir
                    )
                    .padding(.bottom, 8)
                }

                // AI Prompt Optimizer (existing tasks only)
                if case .editing(let taskId) = vm.editorState {
                    PromptOptimizerPanel(
                        vm: vm.promptOptimizerVM,
                        taskId: taskId,
                        onApply: { vm.applyOptimization() }
                    )
                    .padding(.bottom, 8)
                }

                settingsSection("Schedule") { scheduleSection }
                settingsSection("Execution") { executionSection }
                settingsSection("Tags") { tagsSection }
                settingsSection("Environment Variables") { envVarsSection }
            }
            .padding(16)
        }
        .background(Color.inBackground)
        .onAppear {
            if vm.isCreating {
                Task { await vm.promptGeneratorVM.loadAgents() }
            }
        }
    }

    // MARK: - Settings Section Helper

    @ViewBuilder
    private func settingsSection<Content: View>(
        _ title: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title.uppercased())
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .tracking(0.5)
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.panel)
                .stroke(Color.inBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.panel)
    }

    // MARK: - Schedule Section

    @ViewBuilder
    private var scheduleSection: some View {
        ScheduleBuilderView(vm: vm)
    }

    // MARK: - Execution Section

    @ViewBuilder
    private var executionSection: some View {
        INFormField(label: "Working Directory") {
            HStack(spacing: 8) {
                INTextField(
                    text: $vm.draft.workingDir,
                    placeholder: "~/projects/my-repo"
                )
                Button {
                    let panel = NSOpenPanel()
                    panel.canChooseFiles = false
                    panel.canChooseDirectories = true
                    panel.allowsMultipleSelection = false
                    panel.prompt = "Select"
                    panel.message = "Choose a working directory for this task"
                    if panel.runModal() == .OK, let url = panel.url {
                        vm.draft.workingDir = url.path
                    }
                } label: {
                    Image(systemName: "folder")
                        .font(.system(size: 14))
                        .foregroundColor(.inTextMuted)
                }
                .buttonStyle(.plain)
                .padding(.vertical, 10)
                .padding(.horizontal, 10)
                .background(Color.inCodeBackground)
                .overlay(
                    RoundedRectangle(cornerRadius: INRadius.button)
                        .stroke(Color.inBorderInput, lineWidth: 1)
                )
                .cornerRadius(INRadius.button)
                .accessibilityLabel("Browse for working directory")
            }
        }

        INFormField(label: "Timeout (seconds)") {
            INTextField(
                text: Binding(
                    get: { "\(vm.draft.timeoutSecs)" },
                    set: { vm.draft.timeoutSecs = Int($0) ?? 600 }
                ),
                placeholder: "600"
            )
        }
    }

    // MARK: - Tags Section

    @ViewBuilder
    private var tagsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            INTextField(
                text: $tagInput,
                placeholder: "Add a tag and press Enter",
                onSubmit: {
                    let trimmed = tagInput.trimmingCharacters(in: .whitespaces)
                    if !trimmed.isEmpty && vm.draft.tags.count < 20 {
                        vm.draft.tags.append(trimmed)
                        tagInput = ""
                    }
                }
            )
            if !vm.draft.tags.isEmpty {
                FlowLayout(spacing: 4) {
                    ForEach(Array(vm.draft.tags.enumerated()), id: \.offset) { idx, tag in
                        TagChip(text: tag) {
                            vm.draft.tags.remove(at: idx)
                        }
                    }
                }
            }
        }
    }

    // MARK: - Env Vars Section

    @ViewBuilder
    private var envVarsSection: some View {
        if !vm.draft.envVars.isEmpty {
            HStack {
                Text("KEY")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundColor(.inTextFaint)
                    .tracking(0.5)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text("VALUE")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundColor(.inTextFaint)
                    .tracking(0.5)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Spacer().frame(width: 28)
            }
        }

        ForEach(Array(vm.draft.envVars.keys.sorted().enumerated()), id: \.element) { _, key in
            EnvVarRow(
                key: key,
                value: Binding(
                    get: { vm.draft.envVars[key] ?? "" },
                    set: { vm.draft.envVars[key] = $0 }
                ),
                onRemove: { vm.draft.envVars.removeValue(forKey: key) }
            )
        }

        Button {
            vm.draft.envVars["NEW_KEY_\(vm.draft.envVars.count)"] = ""
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "plus")
                    .font(.system(size: 11))
                Text("Add Variable")
                    .font(.inButtonSmall)
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(INToolbarButtonStyle())
    }
}

// MARK: - EnvVarRow

private struct EnvVarRow: View {
    let key: String
    @Binding var value: String
    let onRemove: () -> Void
    @State private var isSecure: Bool = false

    private var looksLikeSecret: Bool {
        let upper = key.uppercased()
        return upper.contains("TOKEN") || upper.contains("SECRET") ||
               upper.contains("KEY") || upper.contains("PASSWORD") ||
               upper.contains("PASS")
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(key)
                .font(.system(size: 11, design: .monospaced))
                .foregroundColor(.inTextMuted)
                .frame(maxWidth: .infinity, alignment: .leading)

            Group {
                if isSecure || looksLikeSecret {
                    SecureField("", text: $value)
                        .textFieldStyle(.plain)
                        .font(.inInput)
                        .foregroundColor(.inTextPrimary)
                } else {
                    TextField("", text: $value)
                        .textFieldStyle(.plain)
                        .font(.inInput)
                        .foregroundColor(.inTextPrimary)
                }
            }
            .padding(.vertical, 6)
            .padding(.horizontal, 8)
            .background(Color.inCodeBackground)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: 1)
            )
            .cornerRadius(INRadius.button)
            .frame(maxWidth: .infinity)

            Button {
                isSecure.toggle()
            } label: {
                Image(systemName: isSecure ? "eye.slash" : "eye")
                    .font(.system(size: 11))
                    .foregroundColor(.inTextMuted)
            }
            .buttonStyle(.plain)
            .frame(width: 20)
            .opacity(looksLikeSecret ? 1 : 0)

            Button(action: onRemove) {
                Image(systemName: "minus.circle")
                    .font(.system(size: 14))
                    .foregroundColor(.inTextMuted)
            }
            .buttonStyle(.plain)
            .frame(width: 20)
        }
    }
}
