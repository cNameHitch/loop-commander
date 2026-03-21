import SwiftUI

struct TaskEditorView: View {
    @StateObject var vm: TaskEditorViewModel
    let onDismiss: () -> Void
    let onSaved: () -> Void

    @EnvironmentObject var daemonMonitor: DaemonMonitor
    @State private var tagInput = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            HStack {
                Text(vm.isNew ? "New Scheduled Task" : "Edit Task")
                    .font(.inHeading)
                    .foregroundColor(.inTextPrimary)
                Spacer()
                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                        .font(.system(size: 14))
                        .foregroundColor(.inTextMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close editor")
            }
            .padding(.bottom, 28)

            // Template picker (only for new tasks)
            if vm.isNew && !vm.templates.isEmpty {
                templatePicker
                    .padding(.bottom, 20)
            }

            // AI prompt generator (only for new tasks)
            if vm.isNew {
                PromptGeneratorPanel(
                    vm: vm.promptGeneratorVM,
                    draft: $vm.draft,
                    workingDir: vm.draft.workingDir
                )
                .padding(.bottom, 20)
            }

            // Form fields
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Task Name
                    INFormField(label: "Task Name") {
                        INTextField(text: $vm.draft.name, placeholder: "e.g., PR Review Sweep")
                    }

                    // Claude Command
                    INFormField(label: "Claude Command") {
                        INTextEditor(text: $vm.draft.command, placeholder: "claude -p 'Your prompt here...'")
                            .frame(minHeight: 80)
                    }

                    // Skill + Working Dir (2-column)
                    HStack(spacing: 16) {
                        INFormField(label: "Skill (optional)") {
                            INTextField(text: $vm.draft.skill,
                                        placeholder: "/review-pr, /loop, etc.")
                        }
                        INFormField(label: "Working Directory") {
                            HStack(spacing: 8) {
                                INTextField(text: $vm.draft.workingDir,
                                            placeholder: "~/projects/my-repo")
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
                    }

                    // Cron + Human-Readable (2-column)
                    HStack(spacing: 16) {
                        INFormField(label: "Cron Schedule") {
                            INTextField(text: $vm.draft.schedule,
                                        placeholder: "*/15 * * * *")
                        }
                        INFormField(label: "Human-Readable") {
                            INTextField(text: $vm.draft.scheduleHuman,
                                        placeholder: "Every 15 minutes")
                        }
                    }

                    // Budget + Timeout (2-column)
                    HStack(spacing: 16) {
                        INFormField(label: "Max Budget per Run ($)") {
                            INTextField(
                                text: Binding(
                                    get: { String(format: "%.1f", vm.draft.maxBudget) },
                                    set: { vm.draft.maxBudget = Double($0) ?? 5.0 }
                                ),
                                placeholder: "5.0"
                            )
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

                    // Tags
                    INFormField(label: "Tags") {
                        VStack(alignment: .leading, spacing: 8) {
                            INTextField(
                                text: $tagInput,
                                placeholder: "Press enter to add tag",
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

                    // Validation errors
                    if !vm.validationErrors.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(vm.validationErrors, id: \.self) { error in
                                HStack(spacing: 6) {
                                    Image(systemName: "exclamationmark.circle.fill")
                                        .font(.system(size: 11))
                                        .foregroundColor(.inRed)
                                    Text(error)
                                        .font(.inCaption)
                                        .foregroundColor(.inRed)
                                }
                            }
                        }
                    }

                    if let error = vm.error {
                        HStack(spacing: 6) {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.inRed)
                            Text(error)
                                .font(.inCaption)
                                .foregroundColor(.inRed)
                        }
                    }
                }
            }

            Spacer(minLength: 28)

            // Footer buttons
            HStack(spacing: 10) {
                Spacer()
                Button("Cancel", action: onDismiss)
                    .buttonStyle(INSecondaryButtonStyle())

                Button(vm.isNew ? "Create Task" : "Save Changes") {
                    Task {
                        if await vm.save() {
                            onSaved()
                            onDismiss()
                        }
                    }
                }
                .buttonStyle(INPrimaryButtonStyle())
                .disabled(vm.isSaving)
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
        .onAppear {
            vm.setClient(daemonMonitor.client)
            Task { await vm.loadTemplates() }
            if vm.isNew {
                Task { await vm.promptGeneratorVM.loadAgents() }
            }
        }
    }

    // MARK: - Template Picker

    @ViewBuilder
    private var templatePicker: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("START FROM TEMPLATE")
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(vm.templates) { template in
                        Button {
                            vm.applyTemplate(template)
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(template.name)
                                    .font(.inBodyMedium)
                                    .foregroundColor(vm.selectedTemplate == template.slug ? .inAccentLight : .inTextPrimary)
                                    .lineLimit(1)
                                Text(template.description)
                                    .font(.inCaption)
                                    .foregroundColor(.inTextMuted)
                                    .lineLimit(2)
                            }
                            .padding(10)
                            .frame(width: 180, alignment: .leading)
                            .background(vm.selectedTemplate == template.slug ? Color.inAccentBgSubtle : Color.inCodeBackground)
                            .overlay(
                                RoundedRectangle(cornerRadius: INRadius.button)
                                    .stroke(
                                        vm.selectedTemplate == template.slug ? Color.inAccent : Color.inBorderInput,
                                        lineWidth: 1
                                    )
                            )
                            .cornerRadius(INRadius.button)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }
}

// MARK: - Reusable Form Components

struct INFormField<Content: View>: View {
    let label: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)
            content
                .accessibilityLabel(label)
        }
    }
}

struct INTextField: View {
    @Binding var text: String
    var placeholder: String = ""
    var onSubmit: (() -> Void)? = nil
    @FocusState private var isFocused: Bool

    var body: some View {
        TextField(placeholder, text: $text)
            .textFieldStyle(.plain)
            .font(.inInput)
            .foregroundColor(.inTextPrimary)
            .padding(.vertical, 10)
            .padding(.horizontal, 12)
            .background(Color.inCodeBackground)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(
                        isFocused ? Color.inAccentFocus : Color.inBorderInput,
                        lineWidth: 1
                    )
            )
            .cornerRadius(INRadius.button)
            .focused($isFocused)
            .onSubmit { onSubmit?() }
    }
}

struct INTextEditor: View {
    @Binding var text: String
    var placeholder: String = ""
    @FocusState private var isFocused: Bool

    var body: some View {
        ZStack(alignment: .topLeading) {
            TextEditor(text: $text)
                .font(.inInput)
                .foregroundColor(.inTextPrimary)
                .scrollContentBackground(.hidden)
                .padding(.vertical, 8)
                .padding(.horizontal, 10)

            if text.isEmpty {
                Text(placeholder)
                    .font(.inInput)
                    .foregroundColor(.inTextFaint)
                    .padding(.vertical, 16)
                    .padding(.horizontal, 14)
                    .allowsHitTesting(false)
            }
        }
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(
                    isFocused ? Color.inAccentFocus : Color.inBorderInput,
                    lineWidth: 1
                )
        )
        .cornerRadius(INRadius.button)
        .focused($isFocused)
    }
}

// MARK: - Flow Layout for Tags

struct FlowLayout: Layout {
    let spacing: CGFloat

    init(spacing: CGFloat = 4) {
        self.spacing = spacing
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = layoutSubviews(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = layoutSubviews(proposal: proposal, subviews: subviews)
        for (index, position) in result.positions.enumerated() {
            subviews[index].place(
                at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y),
                proposal: .unspecified
            )
        }
    }

    private func layoutSubviews(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var maxX: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            if currentX + size.width > maxWidth && currentX > 0 {
                currentX = 0
                currentY += lineHeight + spacing
                lineHeight = 0
            }

            positions.append(CGPoint(x: currentX, y: currentY))
            lineHeight = max(lineHeight, size.height)
            currentX += size.width + spacing
            maxX = max(maxX, currentX)
        }

        return (CGSize(width: maxX, height: currentY + lineHeight), positions)
    }
}
