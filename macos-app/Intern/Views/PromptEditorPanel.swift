import SwiftUI

/// Inline AI prompt editing panel in the task editor settings pane.
///
/// Displayed for both `.creating` and `.editing` editor states — unlike the
/// optimizer (editing only) and generator (creating only), the editor panel
/// is always available because feedback can drive generation from a partial draft.
///
/// Visual language matches `PromptOptimizerPanel` exactly: same section
/// headers, same component styles, same spacing / color / radius tokens.
struct PromptEditorPanel: View {

    @ObservedObject var vm: PromptEditorViewModel
    let draft: INTaskDraft
    let onApply: () -> Void
    let onDiscard: () -> Void

    var body: some View {
        panelContent
    }

    // MARK: - Panel Content

    private var panelContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            feedbackField
            editButton

            if let error = vm.error {
                errorBanner(message: error)
            }

            if vm.isEditing {
                loadingIndicator
            }

            if let result = vm.result {
                Divider()
                    .background(Color.inSeparator)
                    .padding(.vertical, 4)

                resultSection(result: result)
            }
        }
    }

    // MARK: - Feedback Field

    private var feedbackField: some View {
        INFormField(label: "Describe what you want to change") {
            INTextEditor(
                text: $vm.feedbackText,
                placeholder: "e.g., Make the prompt shorter and focus on security concerns..."
            )
            .frame(minHeight: 72)
        }
    }

    // MARK: - Edit Button

    private var editButton: some View {
        HStack {
            Spacer()
            Button {
                Task {
                    await vm.edit(
                        name: draft.name,
                        command: draft.command,
                        schedule: draft.schedule,
                        budget: draft.maxBudget,
                        timeout: draft.timeoutSecs,
                        tags: draft.tags,
                        agents: draft.agents
                    )
                }
            } label: {
                HStack(spacing: 8) {
                    if vm.isEditing {
                        ProgressView()
                            .scaleEffect(0.7)
                            .frame(width: 14, height: 14)
                            .tint(.white)
                    } else {
                        Image(systemName: "pencil.and.sparkles")
                            .font(.system(size: 13))
                    }
                    Text(vm.isEditing ? "Editing..." : "Edit with AI")
                }
            }
            .buttonStyle(INPrimaryButtonStyle())
            .disabled(!vm.canEdit || vm.isEditing)
            .opacity(!vm.canEdit || vm.isEditing ? 0.5 : 1.0)
        }
    }

    // MARK: - Loading Indicator

    private var loadingIndicator: some View {
        HStack(spacing: 10) {
            ProgressView()
                .scaleEffect(0.8)
                .frame(width: 16, height: 16)
            Text("Refining task with Claude... This may take up to 60 seconds.")
                .font(.inCaption)
                .foregroundColor(.inTextMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
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

    // MARK: - Result Section

    @ViewBuilder
    private func resultSection(result: TaskRefineResult) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("EDIT RESULTS")
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)

            confidenceBar(score: result.confidenceScore)
            changesSummaryView(text: result.changesSummary)

            if !result.fieldChanges.isEmpty {
                fieldChangesView(changes: result.fieldChanges)
            }

            commandDiffView(
                original: result.originalCommand,
                refined: result.refinedCommand
            )

            // Action buttons
            HStack(spacing: 8) {
                Spacer()
                Button("Discard") {
                    onDiscard()
                }
                .buttonStyle(INSecondaryButtonStyle())

                Button("Apply Edit") {
                    onApply()
                }
                .buttonStyle(INPrimaryButtonStyle())
            }
        }
    }

    // MARK: - Confidence Bar

    private func confidenceBar(score: Int) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("CONFIDENCE")
                    .font(.inLabel)
                    .foregroundColor(.inTextFaint)
                    .tracking(0.5)
                Spacer()
                Text("\(score)%")
                    .font(.inBodyMedium)
                    .foregroundColor(confidenceColor(for: score))
            }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(Color.inSurfaceRaised)
                        .frame(height: 6)
                    RoundedRectangle(cornerRadius: 3)
                        .fill(confidenceColor(for: score))
                        .frame(width: geo.size.width * CGFloat(score) / 100.0, height: 6)
                }
            }
            .frame(height: 6)

            Text(confidenceLabel(for: score))
                .font(.inCaption)
                .foregroundColor(.inTextMuted)
        }
    }

    private func confidenceColor(for score: Int) -> Color {
        if score >= 80 { return .inGreen }
        if score >= 50 { return .inAccent }
        return .inAmber
    }

    private func confidenceLabel(for score: Int) -> String {
        switch score {
        case 85...100: return "High confidence. Review and apply."
        case 60...84:  return "Moderate confidence. Review carefully before applying."
        case 40...59:  return "Low confidence. Edits are speculative."
        default:       return "Very low confidence. Consider refining your feedback."
        }
    }

    // MARK: - Changes Summary

    private func changesSummaryView(text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("CHANGES SUMMARY")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
            Text(text)
                .font(.inCaption)
                .foregroundColor(.inTextSecondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }

    // MARK: - Field Changes

    private func fieldChangesView(changes: [String: FieldChange]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("FIELD CHANGES")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
            VStack(alignment: .leading, spacing: 4) {
                ForEach(changes.keys.sorted(), id: \.self) { key in
                    if let change = changes[key] {
                        fieldChangeRow(field: key, change: change)
                    }
                }
            }
        }
    }

    private func fieldChangeRow(field: String, change: FieldChange) -> some View {
        HStack(alignment: .top, spacing: 8) {
            TagChip(text: change.`type`.replacingOccurrences(of: "_", with: " "))
            VStack(alignment: .leading, spacing: 2) {
                Text(field.capitalized)
                    .font(.inBodyMedium)
                    .foregroundColor(.inTextPrimary)
                Text(change.reason)
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.inSurfaceContainer)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }

    // MARK: - Command Diff View

    private func commandDiffView(original: String, refined: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("COMMAND CHANGES")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)

            // Only show diff section if command actually changed
            if original == refined {
                Text("Command unchanged.")
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.inCodeBackground)
                    .overlay(
                        RoundedRectangle(cornerRadius: INRadius.button)
                            .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
                    )
                    .cornerRadius(INRadius.button)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    // Original
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 4) {
                            Image(systemName: "minus.circle.fill")
                                .font(.system(size: 10))
                                .foregroundColor(.inRed)
                            Text("ORIGINAL")
                                .font(.inLabel)
                                .foregroundColor(.inTextFaint)
                                .tracking(0.5)
                        }
                        MarkdownPreviewView(text: original)
                            .frame(minHeight: 60, maxHeight: 180)
                    }
                    .padding(10)
                    .background(Color.inRedBg)
                    .overlay(
                        RoundedRectangle(cornerRadius: INRadius.button)
                            .stroke(Color.inRedBorder, lineWidth: INBorder.standard)
                    )
                    .cornerRadius(INRadius.button)

                    // Refined
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 4) {
                            Image(systemName: "plus.circle.fill")
                                .font(.system(size: 10))
                                .foregroundColor(.inGreen)
                            Text("REFINED")
                                .font(.inLabel)
                                .foregroundColor(.inTextFaint)
                                .tracking(0.5)
                        }
                        MarkdownPreviewView(text: refined)
                            .frame(minHeight: 60, maxHeight: 180)
                    }
                    .padding(10)
                    .background(Color.inCodeBackground)
                    .overlay(
                        RoundedRectangle(cornerRadius: INRadius.button)
                            .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
                    )
                    .cornerRadius(INRadius.button)
                }
            }
        }
    }
}
