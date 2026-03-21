import SwiftUI

/// Full AI prompt generation panel embedded in the task editor.
///
/// Matches the visual language of `TaskEditorView` exactly: same section
/// headers, same component types (`LCFormField`, `LCTextEditor`,
/// `LCPrimaryButtonStyle`, `LCSecondaryButtonStyle`, `FlowLayout`,
/// `TagChip`), and the same spacing / color / radius tokens.
struct PromptGeneratorPanel: View {

    @ObservedObject var vm: PromptGeneratorViewModel
    @Binding var draft: LCTaskDraft
    let workingDir: String

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            sectionHeader
            intentField
            agentSection

            if let error = vm.error {
                errorBanner(message: error)
            }

            generateButton

            if let result = vm.result {
                Divider()
                    .background(Color.lcSeparator)
                    .padding(.vertical, 4)

                resultPreview(result: result)
            }
        }
    }

    // MARK: - Section Header

    private var sectionHeader: some View {
        Text("GENERATE WITH AI")
            .font(.lcLabel)
            .foregroundColor(.white.opacity(0.5))
            .textCase(.uppercase)
            .tracking(0.5)
    }

    // MARK: - Intent Field

    private var intentField: some View {
        LCFormField(label: "Describe what you want this task to do") {
            LCTextEditor(
                text: $vm.intent,
                placeholder: "e.g., Review open pull requests every morning and post a comment summarising the diff…"
            )
            .frame(minHeight: 72)
        }
    }

    // MARK: - Agent Section

    private var agentSection: some View {
        LCFormField(label: "Agents") {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    if vm.isLoadingAgents {
                        ProgressView()
                            .scaleEffect(0.65)
                            .frame(width: 14, height: 14)
                    }
                    Spacer()
                    Button {
                        Task { await vm.refreshRegistry() }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 11))
                            Text("Refresh")
                        }
                    }
                    .buttonStyle(LCToolbarButtonStyle())
                    .disabled(vm.isLoadingAgents)
                    .accessibilityLabel("Refresh agent registry")
                }

                AgentPickerView(
                    agents: vm.agents,
                    selectedAgents: $vm.selectedAgents,
                    agentCategories: vm.agentCategories
                )
            }
        }
    }

    // MARK: - Generate Button

    private var generateButton: some View {
        HStack {
            Spacer()
            Button {
                Task { await vm.generate(workingDir: workingDir) }
            } label: {
                HStack(spacing: 8) {
                    if vm.isGenerating {
                        ProgressView()
                            .scaleEffect(0.7)
                            .frame(width: 14, height: 14)
                            .tint(.white)
                    } else {
                        Image(systemName: "sparkles")
                            .font(.system(size: 13))
                    }
                    Text(vm.isGenerating ? "Generating…" : "Generate Prompt")
                }
            }
            .buttonStyle(LCPrimaryButtonStyle())
            .disabled(!vm.canGenerate || vm.isGenerating)
            .opacity(!vm.canGenerate || vm.isGenerating ? 0.5 : 1.0)
        }
    }

    // MARK: - Error Banner

    private func errorBanner(message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 11))
                .foregroundColor(.lcRed)
            Text(message)
                .font(.lcCaption)
                .foregroundColor(.lcRed)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.lcRedBg)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.button)
                .stroke(Color.lcRedBorder, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.button)
    }

    // MARK: - Result Preview

    @ViewBuilder
    private func resultPreview(result: PromptGenerateResult) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Preview header
            Text("GENERATED PREVIEW")
                .font(.lcLabel)
                .foregroundColor(.white.opacity(0.5))
                .textCase(.uppercase)
                .tracking(0.5)

            // Command preview in a bordered code frame
            VStack(alignment: .leading, spacing: 0) {
                MarkdownPreviewView(text: result.command)
                    .frame(minHeight: 140, maxHeight: 240)
            }
            .overlay(
                RoundedRectangle(cornerRadius: LCRadius.button)
                    .stroke(Color.lcBorderInput, lineWidth: LCBorder.standard)
            )
            .cornerRadius(LCRadius.button)

            // Metadata
            resultMetadata(result: result)

            // Use / Regenerate row
            resultActions(result: result)
        }
    }

    @ViewBuilder
    private func resultMetadata(result: PromptGenerateResult) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if !result.name.isEmpty {
                metaRow(label: "NAME", value: result.name)
            }
            if !result.description.isEmpty {
                metaRow(label: "DESCRIPTION", value: result.description)
            }
            if !result.tags.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("TAGS")
                        .font(.lcLabel)
                        .foregroundColor(.lcTextFaint)
                        .tracking(0.5)
                    FlowLayout(spacing: 4) {
                        ForEach(result.tags, id: \.self) { tag in
                            TagChip(text: tag)
                        }
                    }
                }
            }
            if !result.agents.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("AGENTS")
                        .font(.lcLabel)
                        .foregroundColor(.lcTextFaint)
                        .tracking(0.5)
                    FlowLayout(spacing: 4) {
                        ForEach(result.agents, id: \.self) { slug in
                            Text(slug)
                                .font(.lcTag)
                                .foregroundColor(.lcAccentLight)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .background(Color.lcAccentBg)
                                .cornerRadius(LCRadius.badge)
                        }
                    }
                }
            }
        }
        .padding(12)
        .background(Color.lcCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: LCRadius.button)
                .stroke(Color.lcBorderInput, lineWidth: LCBorder.standard)
        )
        .cornerRadius(LCRadius.button)
    }

    private func metaRow(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.lcLabel)
                .foregroundColor(.lcTextFaint)
                .tracking(0.5)
            Text(value)
                .font(.lcCaption)
                .foregroundColor(.lcTextSecondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: - Result Actions

    @ViewBuilder
    private func resultActions(result: PromptGenerateResult) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            // Use This Prompt
            HStack {
                Spacer()
                Button("Use This Prompt") {
                    applyResult(result)
                }
                .buttonStyle(LCPrimaryButtonStyle())
            }

            // Regenerate with feedback
            Divider()
                .background(Color.lcSeparator)

            LCFormField(label: "Feedback for regeneration (optional)") {
                LCTextEditor(
                    text: $vm.feedbackText,
                    placeholder: "e.g., Make the prompt shorter and focus only on security issues…"
                )
                .frame(minHeight: 56)
            }

            HStack {
                Spacer()
                Button {
                    Task { await vm.regenerate(workingDir: workingDir) }
                } label: {
                    HStack(spacing: 6) {
                        if vm.isGenerating {
                            ProgressView()
                                .scaleEffect(0.65)
                                .frame(width: 12, height: 12)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 12))
                        }
                        Text(vm.isGenerating ? "Regenerating…" : "Regenerate")
                    }
                }
                .buttonStyle(LCSecondaryButtonStyle())
                .disabled(vm.isGenerating)
                .opacity(vm.isGenerating ? 0.5 : 1.0)
            }
        }
    }

    // MARK: - Apply Result to Draft

    private func applyResult(_ result: PromptGenerateResult) {
        draft.command = result.command
        if !result.name.isEmpty {
            draft.name = result.name
        }
        // Merge tags without duplication
        let existingTags = Set(draft.tags)
        let newTags = result.tags.filter { !existingTags.contains($0) }
        draft.tags = Array(existingTags) + newTags
        // Replace agents list from result
        draft.agents = result.agents
    }
}
