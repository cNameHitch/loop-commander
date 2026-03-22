import SwiftUI

/// Full AI prompt optimization panel embedded in the task editor.
///
/// Mirrors the visual language of `PromptGeneratorPanel` exactly: same section
/// headers, same component styles (`INFormField`, `INTextEditor`,
/// `INPrimaryButtonStyle`, `INSecondaryButtonStyle`, `FlowLayout`,
/// `TagChip`), and the same spacing / color / radius tokens.
///
/// This panel is only shown for existing tasks (`.editing` editor state).
/// It is mutually exclusive with `PromptGeneratorPanel` which appears only
/// during `.creating`.
struct PromptOptimizerPanel: View {

    @ObservedObject var vm: PromptOptimizerViewModel
    let taskId: String
    let onApply: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            logSummary
            logCountStepper
            focusPicker

            if let error = vm.error {
                errorBanner(message: error)
            }

            analyzeButton

            if vm.isOptimizing {
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

    // MARK: - Section Header


    // MARK: - Log Summary

    @ViewBuilder
    private var logSummary: some View {
        if vm.isLoadingLogs {
            HStack(spacing: 8) {
                ProgressView()
                    .scaleEffect(0.65)
                    .frame(width: 14, height: 14)
                Text("Loading execution history...")
                    .font(.inCaption)
                    .foregroundColor(.inTextMuted)
            }
        } else if vm.hasLogs {
            HStack(spacing: 12) {
                HStack(spacing: 4) {
                    Circle()
                        .fill(Color.inGreen)
                        .frame(width: 6, height: 6)
                    Text("\(vm.successCount) success")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                }
                HStack(spacing: 4) {
                    Circle()
                        .fill(Color.inRed)
                        .frame(width: 6, height: 6)
                    Text("\(vm.failureCount) failed")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                }
                Spacer()
                Text("\(vm.executionLogs.count) runs loaded")
                    .font(.inCaption)
                    .foregroundColor(.inTextFaint)
            }
        } else {
            emptyHistoryNote
        }
    }

    private var emptyHistoryNote: some View {
        HStack(spacing: 8) {
            Image(systemName: "chart.bar.xaxis")
                .font(.system(size: 12))
                .foregroundColor(.inTextFaint)
            Text("No execution history. Run this task at least once to enable optimization.")
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

    // MARK: - Log Count Stepper

    private var logCountStepper: some View {
        INFormField(label: "Runs to analyze") {
            HStack(spacing: 12) {
                Stepper(
                    value: $vm.selectedLogCount,
                    in: 1...50,
                    step: 1
                ) {
                    Text("\(vm.selectedLogCount)")
                        .font(.inInput)
                        .foregroundColor(.inTextPrimary)
                        .frame(minWidth: 28, alignment: .leading)
                }
                .disabled(vm.isOptimizing)
                Spacer()
            }
        }
    }

    // MARK: - Focus Picker

    private var focusPicker: some View {
        INFormField(label: "Optimization focus") {
            Picker("Focus", selection: $vm.optimizationFocus) {
                ForEach(OptimizationFocus.allCases) { focus in
                    Text(focus.displayName).tag(focus)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .disabled(vm.isOptimizing)
            .tint(.inAccentLight)
        }
    }

    // MARK: - Analyze Button

    private var analyzeButton: some View {
        HStack {
            Spacer()
            Button {
                Task { await vm.optimize(taskId: taskId) }
            } label: {
                HStack(spacing: 8) {
                    if vm.isOptimizing {
                        ProgressView()
                            .scaleEffect(0.7)
                            .frame(width: 14, height: 14)
                            .tint(.white)
                    } else {
                        Image(systemName: "wand.and.stars")
                            .font(.system(size: 13))
                    }
                    Text(vm.isOptimizing ? "Analyzing..." : "Analyze \(vm.selectedLogCount) Runs")
                }
            }
            .buttonStyle(INPrimaryButtonStyle())
            .disabled(!vm.canOptimize || vm.isOptimizing)
            .opacity(!vm.canOptimize || vm.isOptimizing ? 0.5 : 1.0)
        }
    }

    // MARK: - Loading Indicator

    private var loadingIndicator: some View {
        HStack(spacing: 10) {
            ProgressView()
                .scaleEffect(0.8)
                .frame(width: 16, height: 16)
            Text("Analyzing execution history... This may take up to 60 seconds.")
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
    private func resultSection(result: PromptOptimizationResult) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Results header
            Text("OPTIMIZATION RESULTS")
                .font(.inLabel)
                .foregroundColor(.inTextMuted)
                .textCase(.uppercase)
                .tracking(0.5)

            confidenceBar(score: result.confidenceScore)
            changesSummaryView(text: result.changesSummary)

            if !result.optimizationCategories.isEmpty {
                categoriesView(categories: result.optimizationCategories)
            }

            optimizedCommandPreview(command: result.optimizedCommand)

            // Apply button
            HStack {
                Spacer()
                Button("Use This Prompt") {
                    onApply()
                }
                .buttonStyle(INPrimaryButtonStyle())
            }

            // Refine section
            refineSection
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
        case 85...100: return "Strong signal from execution history. Review recommended before accepting."
        case 60...84:  return "Some patterns found but history may be limited. Review carefully."
        case 40...59:  return "Insufficient or inconsistent history. Changes are speculative."
        default:       return "Could not identify reliable patterns. Consider running more tasks first."
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

    // MARK: - Categories

    private func categoriesView(categories: [String]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("CATEGORIES")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
            FlowLayout(spacing: 4) {
                ForEach(categories, id: \.self) { category in
                    TagChip(text: category.capitalized)
                }
            }
        }
    }

    // MARK: - Optimized Command Preview

    private func optimizedCommandPreview(command: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("OPTIMIZED COMMAND PREVIEW")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
            VStack(alignment: .leading, spacing: 0) {
                MarkdownPreviewView(text: command)
                    .frame(minHeight: 120, maxHeight: 240)
            }
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
            )
            .cornerRadius(INRadius.button)
        }
    }

    // MARK: - Refine Section

    private var refineSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Divider()
                .background(Color.inSeparator)

            INFormField(label: "Feedback for re-optimization (optional)") {
                INTextEditor(
                    text: $vm.feedbackText,
                    placeholder: "e.g., Preserve the JSON output format and keep the scope narrow..."
                )
                .frame(minHeight: 56)
            }

            HStack {
                Spacer()
                Button {
                    Task { await vm.reoptimize() }
                } label: {
                    HStack(spacing: 6) {
                        if vm.isOptimizing {
                            ProgressView()
                                .scaleEffect(0.65)
                                .frame(width: 12, height: 12)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 12))
                        }
                        Text(vm.isOptimizing ? "Re-optimizing..." : "Re-optimize with Feedback")
                    }
                }
                .buttonStyle(INSecondaryButtonStyle())
                .disabled(vm.isOptimizing)
                .opacity(vm.isOptimizing ? 0.5 : 1.0)
            }
        }
    }
}
