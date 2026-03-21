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
                    .font(.lcHeading)
                    .foregroundColor(.lcTextPrimary)
                Spacer()
                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                        .font(.system(size: 14))
                        .foregroundColor(.lcTextMuted)
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
                    LCFormField(label: "Task Name") {
                        LCTextField(text: $vm.draft.name, placeholder: "e.g., PR Review Sweep")
                    }

                    // Claude Command
                    LCFormField(label: "Claude Command") {
                        LCTextEditor(text: $vm.draft.command, placeholder: "claude -p 'Your prompt here...'")
                            .frame(minHeight: 80)
                    }

                    // Skill + Working Dir (2-column)
                    HStack(spacing: 16) {
                        LCFormField(label: "Skill (optional)") {
                            LCTextField(text: $vm.draft.skill,
                                        placeholder: "/review-pr, /loop, etc.")
                        }
                        LCFormField(label: "Working Directory") {
                            HStack(spacing: 8) {
                                LCTextField(text: $vm.draft.workingDir,
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
                    }

                    // Cron + Human-Readable (2-column)
                    HStack(spacing: 16) {
                        LCFormField(label: "Cron Schedule") {
                            LCTextField(text: $vm.draft.schedule,
                                        placeholder: "*/15 * * * *")
                        }
                        LCFormField(label: "Human-Readable") {
                            LCTextField(text: $vm.draft.scheduleHuman,
                                        placeholder: "Every 15 minutes")
                        }
                    }

                    // Budget + Timeout (2-column)
                    HStack(spacing: 16) {
                        LCFormField(label: "Max Budget per Run ($)") {
                            LCTextField(
                                text: Binding(
                                    get: { String(format: "%.1f", vm.draft.maxBudget) },
                                    set: { vm.draft.maxBudget = Double($0) ?? 5.0 }
                                ),
                                placeholder: "5.0"
                            )
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

                    // Tags
                    LCFormField(label: "Tags") {
                        VStack(alignment: .leading, spacing: 8) {
                            LCTextField(
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
                                        .foregroundColor(.lcRed)
                                    Text(error)
                                        .font(.lcCaption)
                                        .foregroundColor(.lcRed)
                                }
                            }
                        }
                    }

                    if let error = vm.error {
                        HStack(spacing: 6) {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.lcRed)
                            Text(error)
                                .font(.lcCaption)
                                .foregroundColor(.lcRed)
                        }
                    }
                }
            }

            Spacer(minLength: 28)

            // Footer buttons
            HStack(spacing: 10) {
                Spacer()
                Button("Cancel", action: onDismiss)
                    .buttonStyle(LCSecondaryButtonStyle())

                Button(vm.isNew ? "Create Task" : "Save Changes") {
                    Task {
                        if await vm.save() {
                            onSaved()
                            onDismiss()
                        }
                    }
                }
                .buttonStyle(LCPrimaryButtonStyle())
                .disabled(vm.isSaving)
            }
        }
        .padding(32)
        .frame(width: 560)
        .background(Color.lcSurface)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.modal)
                .stroke(Color.lcSeparator, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.modal)
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
                .font(.lcLabel)
                .foregroundColor(.white.opacity(0.5))
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
                                    .font(.lcBodyMedium)
                                    .foregroundColor(vm.selectedTemplate == template.slug ? .lcAccentLight : .lcTextPrimary)
                                    .lineLimit(1)
                                Text(template.description)
                                    .font(.lcCaption)
                                    .foregroundColor(.lcTextMuted)
                                    .lineLimit(2)
                            }
                            .padding(10)
                            .frame(width: 180, alignment: .leading)
                            .background(vm.selectedTemplate == template.slug ? Color.lcAccentBgSubtle : Color.lcCodeBackground)
                            .overlay(
                                RoundedRectangle(cornerRadius: LCRadius.button)
                                    .stroke(
                                        vm.selectedTemplate == template.slug ? Color.lcAccent : Color.lcBorderInput,
                                        lineWidth: 1
                                    )
                            )
                            .cornerRadius(LCRadius.button)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }
}

// MARK: - Reusable Form Components

struct LCFormField<Content: View>: View {
    let label: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.lcLabel)
                .foregroundColor(.white.opacity(0.5))
                .textCase(.uppercase)
                .tracking(0.5)
            content
                .accessibilityLabel(label)
        }
    }
}

struct LCTextField: View {
    @Binding var text: String
    var placeholder: String = ""
    var onSubmit: (() -> Void)? = nil
    @FocusState private var isFocused: Bool

    var body: some View {
        TextField(placeholder, text: $text)
            .textFieldStyle(.plain)
            .font(.lcInput)
            .foregroundColor(.lcTextPrimary)
            .padding(.vertical, 10)
            .padding(.horizontal, 12)
            .background(Color.lcCodeBackground)
            .overlay(
                RoundedRectangle(cornerRadius: LCRadius.button)
                    .stroke(
                        isFocused ? Color.lcAccentFocus : Color.lcBorderInput,
                        lineWidth: 1
                    )
            )
            .cornerRadius(LCRadius.button)
            .focused($isFocused)
            .onSubmit { onSubmit?() }
    }
}

struct LCTextEditor: View {
    @Binding var text: String
    var placeholder: String = ""
    @FocusState private var isFocused: Bool

    var body: some View {
        ZStack(alignment: .topLeading) {
            TextEditor(text: $text)
                .font(.lcInput)
                .foregroundColor(.lcTextPrimary)
                .scrollContentBackground(.hidden)
                .padding(.vertical, 8)
                .padding(.horizontal, 10)

            if text.isEmpty {
                Text(placeholder)
                    .font(.lcInput)
                    .foregroundColor(.lcTextFaint)
                    .padding(.vertical, 16)
                    .padding(.horizontal, 14)
                    .allowsHitTesting(false)
            }
        }
        .background(Color.lcCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.button)
                .stroke(
                    isFocused ? Color.lcAccentFocus : Color.lcBorderInput,
                    lineWidth: 1
                )
        )
        .cornerRadius(LCRadius.button)
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
