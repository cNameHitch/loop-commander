import SwiftUI

/// Full AI prompt generation panel embedded in the task editor.
///
/// Matches the visual language of `TaskEditorView` exactly: same section
/// headers, same component types (`INFormField`, `INTextEditor`,
/// `INPrimaryButtonStyle`, `INSecondaryButtonStyle`, `FlowLayout`,
/// `TagChip`), and the same spacing / color / radius tokens.
struct PromptGeneratorPanel: View {

    @ObservedObject var vm: PromptGeneratorViewModel
    @Binding var draft: INTaskDraft
    let workingDir: String

    /// Controls whether the AGENTS disclosure section is expanded.
    /// Collapsed by default — most tasks use the standard Claude Code agent.
    @State private var agentSectionExpanded: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            sectionHeader
            intentField
            agentDisclosure

            if let error = vm.error {
                errorBanner(message: error)
            }

            generateButton

            if let result = vm.result {
                Divider()
                    .background(Color.inSeparator)
                    .padding(.vertical, 4)

                resultPreview(result: result)
            }
        }
    }

    // MARK: - Section Header

    private var sectionHeader: some View {
        Text("GENERATE WITH AI")
            .font(.inLabel)
            .foregroundColor(.inTextMuted)
            .textCase(.uppercase)
            .tracking(0.5)
    }

    // MARK: - Intent Field

    private var intentField: some View {
        INFormField(label: "Describe what you want this task to do") {
            INTextEditor(
                text: $vm.intent,
                placeholder: "e.g., Review open pull requests every morning and post a comment summarising the diff…"
            )
            .frame(minHeight: 72)
        }
    }

    // MARK: - Agent Section

    /// Custom disclosure header for the AGENTS section.
    ///
    /// Replaces the plain `DisclosureGroup` with a hand-rolled toggle so we
    /// can control hover state, chevron animation, and border independently.
    /// All tokens are sourced from the existing design system — no new values
    /// are introduced.
    @State private var agentHeaderHovered: Bool = false

    private var agentDisclosure: some View {
        VStack(alignment: .leading, spacing: 0) {
            agentDisclosureHeader
            if agentSectionExpanded {
                agentDisclosureContent
                    .transition(.inFadeSlide)
            }
        }
        .animation(.inFadeSlide, value: agentSectionExpanded)
    }

    /// The tappable pill row: chevron + "AGENTS" label + "(optional)" suffix.
    private var agentDisclosureHeader: some View {
        Button {
            agentSectionExpanded.toggle()
        } label: {
            HStack(spacing: INSpacing.xs) {
                // Chevron: rotates 90 deg when expanded.
                Image(systemName: "chevron.right")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundColor(
                        agentSectionExpanded
                            ? Color.inAccent
                            : (agentHeaderHovered ? Color.inTextSecondary : Color.inTextMuted)
                    )
                    .rotationEffect(.degrees(agentSectionExpanded ? 90 : 0))
                    .animation(.inQuick, value: agentSectionExpanded)

                // Primary label — identical font to all other section headers.
                Text("AGENTS")
                    .font(.inLabel)
                    .foregroundColor(
                        agentSectionExpanded || agentHeaderHovered
                            ? Color.inTextSecondary
                            : Color.inTextMuted
                    )
                    .tracking(0.5)
                    .animation(.inQuick, value: agentSectionExpanded)
                    .animation(.inQuick, value: agentHeaderHovered)

                // Optional suffix — visually lighter than the primary label,
                // co-located so it reads in one pass without adding structure.
                Text("(optional)")
                    .font(.system(size: 10, weight: .regular))
                    .foregroundColor(Color.inTextSubtle)
            }
            .padding(.vertical, INSpacing.md)
            .padding(.horizontal, INSpacing.lg)
            .background(agentHeaderHovered ? Color.inSurfaceRaised : Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.filter)
                    .stroke(
                        agentSectionExpanded
                            ? Color.inBorder
                            : (agentHeaderHovered
                                ? Color.white.opacity(0.18)
                                : Color.inBorderInput),
                        lineWidth: INBorder.standard
                    )
            )
            .cornerRadius(INRadius.filter)
            .animation(.inQuick, value: agentHeaderHovered)
            .animation(.inQuick, value: agentSectionExpanded)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            agentHeaderHovered = hovering
        }
        .accessibilityLabel("Agents section, \(agentSectionExpanded ? "expanded" : "collapsed"), optional")
        .accessibilityHint("Activate to \(agentSectionExpanded ? "collapse" : "expand") agent picker")
        .accessibilityAddTraits(.isButton)
    }

    /// The expanded content: refresh toolbar + agent picker list.
    private var agentDisclosureContent: some View {
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
                .buttonStyle(INToolbarButtonStyle())
                .disabled(vm.isLoadingAgents)
                .accessibilityLabel("Refresh agent registry")
            }

            AgentPickerView(
                agents: vm.agents,
                selectedAgents: $vm.selectedAgents,
                agentCategories: vm.agentCategories
            )
        }
        .padding(.top, 8)
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
            .buttonStyle(INPrimaryButtonStyle())
            .disabled(!vm.canGenerate || vm.isGenerating)
            .opacity(!vm.canGenerate || vm.isGenerating ? 0.5 : 1.0)
        }
    }

    // MARK: - Error Banner

    private func errorBanner(message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 11))
                .foregroundColor(.inRed)
            Text(message)
                .font(.inCaption)
                .foregroundColor(.inRed)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inRedBg)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inRedBorder, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }

    // MARK: - Result Preview

    @ViewBuilder
    private func resultPreview(result: PromptGenerateResult) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Preview header
            Text("GENERATED PREVIEW")
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)

            // Command preview in a bordered code frame
            VStack(alignment: .leading, spacing: 0) {
                MarkdownPreviewView(text: result.command)
                    .frame(minHeight: 140, maxHeight: 240)
            }
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
            )
            .cornerRadius(INRadius.button)

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
                        .font(.inLabel)
                        .foregroundColor(.inTextFaint)
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
                        .font(.inLabel)
                        .foregroundColor(.inTextFaint)
                        .tracking(0.5)
                    FlowLayout(spacing: 4) {
                        ForEach(result.agents, id: \.self) { slug in
                            Text(slug)
                                .font(.inTag)
                                .foregroundColor(.inAccentLight)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .background(Color.inAccentBg)
                                .cornerRadius(INRadius.badge)
                        }
                    }
                }
            }
        }
        .padding(12)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }

    private func metaRow(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
            Text(value)
                .font(.inCaption)
                .foregroundColor(.inTextSecondary)
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
                .buttonStyle(INPrimaryButtonStyle())
            }

            // Regenerate with feedback
            Divider()
                .background(Color.inSeparator)

            INFormField(label: "Feedback for regeneration (optional)") {
                INTextEditor(
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
                .buttonStyle(INSecondaryButtonStyle())
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
