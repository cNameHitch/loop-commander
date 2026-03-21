import SwiftUI

struct EditorView: View {
    @ObservedObject var vm: EditorViewModel
    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @EnvironmentObject var taskListVM: TaskListViewModel
    @State private var tagInput: String = ""
    @State private var discoveredCommands: [ClaudeCommand] = []
    @State private var skillSearchText: String = ""
    @State private var showPreview: Bool = false

    private var filteredCommands: [ClaudeCommand] {
        if skillSearchText.isEmpty { return discoveredCommands }
        let query = skillSearchText.lowercased()
        return discoveredCommands.filter {
            $0.name.lowercased().contains(query) ||
            $0.description.lowercased().contains(query) ||
            $0.projectName.lowercased().contains(query)
        }
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
                    Text("Editor")
                        .font(.lcHeadingLarge)
                        .foregroundColor(.lcTextPrimary)
                    Text("Create a new task or edit an existing one")
                        .font(.lcCaption)
                        .foregroundColor(.lcTextSubtle)
                }
                Spacer()
                Button("+ New Task") {
                    vm.startNewTask()
                }
                .buttonStyle(LCPrimaryButtonStyle())
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            .background(Color.lcSurface)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(Color.lcSeparator)
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
            .background(Color.lcBackground)
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
                .font(.lcLabel)
                .foregroundColor(.white.opacity(0.5))
                .tracking(0.5)

            if taskListVM.tasks.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "tray")
                        .foregroundColor(.lcTextFaint)
                    Text("No tasks yet. Create one to get started.")
                        .font(.lcCaption)
                        .foregroundColor(.lcTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.lcSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: LCRadius.panel)
                        .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
                )
                .cornerRadius(LCRadius.panel)
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

    private func taskCard(_ task: LCTask) -> some View {
        HStack(spacing: 12) {
            // Status dot
            Circle()
                .fill(statusColor(for: task.status))
                .frame(width: 8, height: 8)

            // Task info
            VStack(alignment: .leading, spacing: 2) {
                Text(task.name)
                    .font(.lcBodyMedium)
                    .foregroundColor(.lcTextPrimary)
                    .lineLimit(1)
                Text(task.scheduleHuman)
                    .font(.lcCaption)
                    .foregroundColor(.lcTextMuted)
            }

            Spacer()

            // Status label
            Text(task.status.rawValue.capitalized)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(statusColor(for: task.status))
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(statusColor(for: task.status).opacity(0.15))
                .cornerRadius(LCRadius.badge)

            // Edit button
            Button("Edit") {
                NotificationCenter.default.post(
                    name: .editorOpenTask,
                    object: nil,
                    userInfo: ["task": task]
                )
            }
            .buttonStyle(LCToolbarButtonStyle())
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.lcSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.panel)
                .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.panel)
    }

    // MARK: - Status Color

    private func statusColor(for status: TaskStatus) -> Color {
        switch status {
        case .active:
            return .lcGreen
        case .paused:
            return .lcAmber
        case .error:
            return .lcRed
        case .running:
            return .lcAccent
        case .disabled:
            return .lcTextFaint
        }
    }

    // MARK: - Skills List Section

    private var skillsListSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("DISCOVERED SKILLS")
                    .font(.lcLabel)
                    .foregroundColor(.white.opacity(0.5))
                    .tracking(0.5)
                Spacer()
                if !discoveredCommands.isEmpty {
                    Text("\(filteredCommands.count) of \(discoveredCommands.count)")
                        .font(.lcCaption)
                        .foregroundColor(.lcTextMuted)
                }
            }

            if !discoveredCommands.isEmpty {
                LCTextField(
                    text: $skillSearchText,
                    placeholder: "Filter skills..."
                )
            }

            if discoveredCommands.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.lcTextFaint)
                    Text("No Claude Code commands found. Add .md files to .claude/commands/ in your projects.")
                        .font(.lcCaption)
                        .foregroundColor(.lcTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.lcSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: LCRadius.panel)
                        .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
                )
                .cornerRadius(LCRadius.panel)
            } else if filteredCommands.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.lcTextFaint)
                    Text("No skills match \"\(skillSearchText)\"")
                        .font(.lcCaption)
                        .foregroundColor(.lcTextMuted)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.lcSurfaceContainer)
                .overlay(
                    RoundedRectangle(cornerRadius: LCRadius.panel)
                        .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
                )
                .cornerRadius(LCRadius.panel)
            } else {
                VStack(spacing: 4) {
                    ForEach(filteredCommands) { command in
                        skillCard(command)
                    }
                }
            }
        }
    }

    // MARK: - Skill Card

    private func skillCard(_ command: ClaudeCommand) -> some View {
        HStack(spacing: 12) {
            // Skill icon
            Image(systemName: "terminal")
                .font(.system(size: 14))
                .foregroundColor(.lcAccentLight)
                .frame(width: 24)

            // Skill info
            VStack(alignment: .leading, spacing: 2) {
                Text("/\(command.name)")
                    .font(.lcBodyMedium)
                    .foregroundColor(.lcTextPrimary)
                    .lineLimit(1)
                if !command.description.isEmpty {
                    Text(command.description)
                        .font(.lcCaption)
                        .foregroundColor(.lcTextMuted)
                        .lineLimit(2)
                }
            }

            Spacer()

            // Project tag
            Text(command.projectName)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(.lcAccentLight)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(Color.lcTagBg)
                .cornerRadius(LCRadius.badge)

            // Open button
            Button("Open") {
                NotificationCenter.default.post(
                    name: .editorOpenImport,
                    object: nil,
                    userInfo: ["command": command]
                )
            }
            .buttonStyle(LCToolbarButtonStyle())
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.lcSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.panel)
                .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.panel)
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
                                .foregroundColor(.lcRed)
                            Text(err)
                                .font(.lcCaption)
                                .foregroundColor(.lcRed)
                        }
                    }
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 8)
                .background(Color.lcBackground)
            }

            if let error = vm.error {
                HStack(spacing: 6) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.lcRed)
                    Text(error)
                        .font(.lcCaption)
                        .foregroundColor(.lcRed)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 8)
                .background(Color.lcBackground)
            }
        }
        .animation(.lcQuick, value: vm.isDirty)
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
                .font(.lcHeadingLarge)
                .textFieldStyle(.plain)
                .foregroundColor(.lcTextPrimary)
                .accessibilityLabel("Task name")

            Spacer()

            Button("Discard") {
                if vm.isDirty {
                    vm.confirmDiscard()
                } else {
                    vm.editorState = .empty
                }
            }
            .buttonStyle(LCSecondaryButtonStyle())
            .keyboardShortcut(.escape, modifiers: [])

            Button(vm.isCreating ? "Create Task" : "Save Changes") {
                Task { await vm.save() }
            }
            .buttonStyle(LCPrimaryButtonStyle())
            .disabled(vm.isSaving)
            .keyboardShortcut("s", modifiers: .command)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
        .background(Color.lcSurface)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.lcSeparator)
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
                    .foregroundColor(.lcAmber)
                Text("Unsaved changes -- you have edits in progress.")
                    .font(.lcBodyMedium)
                    .foregroundColor(.lcAmber)
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(Color.lcAmberBg)
            .overlay(alignment: .top) {
                Rectangle()
                    .fill(Color.lcAmber.opacity(0.2))
                    .frame(height: 1)
            }
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(Color.lcSeparator)
                    .frame(height: 1)
            }
            .transition(.lcFadeSlide)
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
                    .fill(Color.lcSeparator)
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
                    .font(.lcLabel)
                    .foregroundColor(.white.opacity(0.5))
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
                LCTextEditor(
                    text: $vm.draft.command,
                    placeholder: "claude -p 'Your prompt here...'"
                )
                .frame(maxHeight: .infinity)
            }

            Rectangle()
                .fill(Color.lcBorder)
                .frame(height: 1)
            HStack {
                Text("\(vm.draft.command.count) characters")
                    .font(.lcCaption)
                    .foregroundColor(.lcTextMuted)
                Spacer()
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
        }
        .background(Color.lcBackground)
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
        .background(Color.lcCodeBackground)
        .cornerRadius(LCRadius.filter)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.filter)
                .stroke(Color.lcBorderInput, lineWidth: 1)
        )
    }

    private func toggleSegment(
        label: String,
        isActive: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(label)
                .font(.lcButtonSmall)
                .foregroundColor(isActive ? .lcAccentLight : .lcTextMuted)
                .padding(.vertical, 5)
                .padding(.horizontal, 10)
                .background(isActive ? Color.lcAccentBg : Color.clear)
                .cornerRadius(LCRadius.filter)
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

                settingsSection("Schedule") { scheduleSection }
                settingsSection("Execution") { executionSection }
                settingsSection("Tags") { tagsSection }
                settingsSection("Environment Variables") { envVarsSection }
            }
            .padding(16)
        }
        .background(Color.lcBackground)
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
                .font(.lcLabel)
                .foregroundColor(.white.opacity(0.5))
                .tracking(0.5)
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.lcSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.panel)
                .stroke(Color.lcBorder, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.panel)
    }

    // MARK: - Schedule Section

    @ViewBuilder
    private var scheduleSection: some View {
        ScheduleBuilderView(vm: vm)
    }

    // MARK: - Execution Section

    @ViewBuilder
    private var executionSection: some View {
        LCFormField(label: "Working Directory") {
            HStack(spacing: 8) {
                LCTextField(
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
                        .foregroundColor(.lcTextMuted)
                }
                .buttonStyle(.plain)
                .padding(.vertical, 10)
                .padding(.horizontal, 10)
                .background(Color.lcCodeBackground)
                .overlay(
                    RoundedRectangle(cornerRadius: LCRadius.button)
                        .stroke(Color.lcBorderInput, lineWidth: 1)
                )
                .cornerRadius(LCRadius.button)
                .accessibilityLabel("Browse for working directory")
            }
        }

        LCFormField(label: "Timeout (seconds)") {
            LCTextField(
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
            LCTextField(
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
                    .foregroundColor(.lcTextFaint)
                    .tracking(0.5)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text("VALUE")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundColor(.lcTextFaint)
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
                    .font(.lcButtonSmall)
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(LCToolbarButtonStyle())
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
                .foregroundColor(.lcTextMuted)
                .frame(maxWidth: .infinity, alignment: .leading)

            Group {
                if isSecure || looksLikeSecret {
                    SecureField("", text: $value)
                        .textFieldStyle(.plain)
                        .font(.lcInput)
                        .foregroundColor(.lcTextPrimary)
                } else {
                    TextField("", text: $value)
                        .textFieldStyle(.plain)
                        .font(.lcInput)
                        .foregroundColor(.lcTextPrimary)
                }
            }
            .padding(.vertical, 6)
            .padding(.horizontal, 8)
            .background(Color.lcCodeBackground)
            .overlay(
                RoundedRectangle(cornerRadius: LCRadius.button)
                    .stroke(Color.lcBorderInput, lineWidth: 1)
            )
            .cornerRadius(LCRadius.button)
            .frame(maxWidth: .infinity)

            Button {
                isSecure.toggle()
            } label: {
                Image(systemName: isSecure ? "eye.slash" : "eye")
                    .font(.system(size: 11))
                    .foregroundColor(.lcTextMuted)
            }
            .buttonStyle(.plain)
            .frame(width: 20)
            .opacity(looksLikeSecret ? 1 : 0)

            Button(action: onRemove) {
                Image(systemName: "minus.circle")
                    .font(.system(size: 14))
                    .foregroundColor(.lcTextMuted)
            }
            .buttonStyle(.plain)
            .frame(width: 20)
        }
    }
}
