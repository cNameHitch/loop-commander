import Foundation
import SwiftUI

// MARK: - Editor State

enum EditorState: Equatable {
    case empty
    case creating
    case editing(taskId: String)
}

// MARK: - EditorViewModel

/// ViewModel that owns the persistent editor tab state for Loop Commander.
///
/// Unlike the modal `TaskEditorViewModel`, this object lives for the lifetime
/// of the app and supports the inline editor panel, tracking dirty state,
/// preset-based schedule builders, and save/discard semantics.
@MainActor
class EditorViewModel: ObservableObject {

    // MARK: - Published State

    /// The mutable draft that the editor form binds to.
    @Published var draft: LCTaskDraft = LCTaskDraft()

    /// Whether we are empty, creating a new task, or editing an existing one.
    @Published var editorState: EditorState = .empty

    /// True while the async save RPC call is in flight.
    @Published var isSaving: Bool = false

    /// Non-nil when a save or validation error should be shown.
    @Published var error: String?

    /// Human-readable validation messages; empty when the draft is valid.
    @Published var validationErrors: [String] = []

    /// The selected schedule preset; drives the cron builder pickers.
    @Published var schedulePreset: SchedulePreset = .every15Min

    /// Hour component (0–23) used by time-requiring presets.
    @Published var selectedHour: Int = 9

    /// Minute component (0–59) used by time-requiring presets.
    @Published var selectedMinute: Int = 0

    /// Set of weekday indices (0=Sun … 6=Sat) used by `.weeklyOn`.
    @Published var selectedWeekdays: Set<Int> = [1]

    /// Day-of-month (1–31) used by `.monthlyOn`.
    @Published var selectedDayOfMonth: Int = 1

    /// Controls the "You have unsaved changes – discard?" alert.
    @Published var showDiscardAlert: Bool = false

    /// Briefly true after a successful save to show a confirmation banner.
    @Published var showSavedConfirmation: Bool = false

    /// Child view model for AI prompt generation.
    @Published var promptGeneratorVM = PromptGeneratorViewModel()

    /// Child view model for AI prompt optimization.
    @Published var promptOptimizerVM = PromptOptimizerViewModel()

    // MARK: - Internal State

    /// Snapshot of the draft as it existed when editing began; used for dirty
    /// comparison and for restoring on discard.
    private var originalSnapshot: LCTaskDraft?

    /// The stable id of the task being edited, when `editorState == .editing`.
    private var taskId: String?

    /// JSON-RPC client injected from the surrounding environment.
    private var client: DaemonClient?

    // MARK: - Computed Properties

    /// True when the current draft differs from the baseline (empty for new
    /// tasks, original snapshot for edits).
    var isDirty: Bool {
        switch editorState {
        case .empty:
            return false
        case .creating:
            return draft != LCTaskDraft()
        case .editing:
            guard let snapshot = originalSnapshot else { return false }
            return draft != snapshot
        }
    }

    /// Convenience flag so views can conditionalize "Create" vs "Save" labels.
    var isCreating: Bool {
        if case .creating = editorState { return true }
        return false
    }

    // MARK: - Client Injection

    /// Inject the daemon client.  Called from EditorView.onAppear via the
    /// DaemonMonitor environment object.
    func setClient(_ client: DaemonClient) {
        self.client = client
        promptGeneratorVM.setClient(client)
        promptOptimizerVM.setClient(client)
    }

    // MARK: - Lifecycle

    /// Prepare the editor to create a brand-new task.
    func startNewTask() {
        editorState = .creating
        draft = LCTaskDraft()
        originalSnapshot = nil
        taskId = nil
        schedulePreset = .every15Min
        selectedHour = 9
        selectedMinute = 0
        selectedWeekdays = [1]
        selectedDayOfMonth = 1
        error = nil
        validationErrors = []
    }

    /// Load an existing task into the editor for modification.
    func loadTask(_ task: LCTask) {
        editorState = .editing(taskId: task.id)
        let d = LCTaskDraft(from: task)
        draft = d
        originalSnapshot = d
        taskId = task.id
        error = nil
        validationErrors = []
        inferPresetFromCron(draft.schedule)
        Task { await promptOptimizerVM.loadLogs(taskId: task.id) }
    }

    /// Pre-populate a new-task draft from a discovered Claude Code command.
    func loadFromImportedCommand(_ command: ClaudeCommand) {
        editorState = .creating
        let d = LCTaskDraft(from: command)
        draft = d
        originalSnapshot = nil
        taskId = nil
        error = nil
        validationErrors = []
        inferPresetFromCron(draft.schedule)
    }

    // MARK: - Validation

    /// Validate the current draft, populating `validationErrors`.
    ///
    /// - Returns: `true` when validation passes (no errors).
    @discardableResult
    func validate() -> Bool {
        validationErrors = []

        if draft.name.trimmingCharacters(in: .whitespaces).isEmpty {
            validationErrors.append("Task name is required")
        }
        if draft.name.count > 200 {
            validationErrors.append("Task name must be 200 characters or fewer")
        }
        if draft.command.trimmingCharacters(in: .whitespaces).isEmpty {
            validationErrors.append("Command is required")
        }
        if draft.timeoutSecs <= 0 {
            validationErrors.append("Timeout must be greater than 0")
        }
        if draft.timeoutSecs > 86400 {
            validationErrors.append("Timeout must be 86400 seconds or less")
        }
        if draft.tags.count > 20 {
            validationErrors.append("Maximum 20 tags allowed")
        }

        return validationErrors.isEmpty
    }

    // MARK: - Save

    /// Persist the current draft via the daemon RPC layer.
    ///
    /// - Returns: `true` on success, `false` on validation failure or RPC error.
    func save() async -> Bool {
        guard validate() else { return false }
        guard client != nil else { return false }

        isSaving = true
        error = nil

        do {
            if isCreating {
                _ = try await client!.createTask(draft.toCreateInput())
            } else {
                _ = try await client!.updateTask(draft.toUpdateInput(id: taskId!))
                // After a successful edit, the new snapshot becomes the current draft.
                originalSnapshot = draft
            }
            isSaving = false
            showSavedConfirmation = true
            NotificationCenter.default.post(name: .refreshData, object: nil)
            // Reset editor to empty state after successful save
            editorState = .empty
            draft = LCTaskDraft()
            originalSnapshot = nil
            taskId = nil
            return true
        } catch {
            self.error = error.localizedDescription
            isSaving = false
            return false
        }
    }

    // MARK: - Discard

    /// Show the discard-changes confirmation alert.
    func confirmDiscard() {
        showDiscardAlert = true
    }

    /// Apply the optimizer's result to the draft command.
    func applyOptimization() {
        promptOptimizerVM.applyOptimization(to: &draft)
    }

    /// Perform the actual discard action – called when the user confirms the alert.
    func discard() {
        promptOptimizerVM.reset()
        showDiscardAlert = false
        switch editorState {
        case .empty:
            break
        case .creating:
            editorState = .empty
            draft = LCTaskDraft()
            originalSnapshot = nil
            taskId = nil
        case .editing:
            if let snapshot = originalSnapshot {
                draft = snapshot
            }
        }
        validationErrors = []
        error = nil
        inferPresetFromCron(draft.schedule)
    }

    // MARK: - Schedule Preset Sync

    /// Write the cron expression and human description back into the draft based
    /// on the current preset + picker selections.  No-ops when the preset is
    /// `.custom` to avoid overwriting a hand-edited expression.
    func syncCronFromPreset() {
        guard schedulePreset != .custom else { return }
        draft.schedule = schedulePreset.cronExpression(
            hour: selectedHour,
            minute: selectedMinute,
            weekdays: selectedWeekdays,
            dayOfMonth: selectedDayOfMonth
        )
        draft.scheduleHuman = schedulePreset.humanDescription(
            hour: selectedHour,
            minute: selectedMinute,
            weekdays: selectedWeekdays,
            dayOfMonth: selectedDayOfMonth
        )
    }

    /// Toggle a single weekday in `selectedWeekdays`.
    ///
    /// Removal is a no-op when it would leave the set empty, ensuring at least
    /// one weekday is always selected.  Calls `syncCronFromPreset()` after any
    /// change.
    func toggleWeekday(_ index: Int) {
        if selectedWeekdays.contains(index) {
            guard selectedWeekdays.count > 1 else { return }
            selectedWeekdays.remove(index)
        } else {
            selectedWeekdays.insert(index)
        }
        syncCronFromPreset()
    }

    // MARK: - Cron Inference

    /// Parse `cron` and update `schedulePreset` plus the sub-picker properties
    /// so the UI reflects the loaded schedule as closely as possible.
    ///
    /// Matches from most-specific to least-specific; falls back to `.custom`
    /// for any expression that doesn't match a known pattern.
    func inferPresetFromCron(_ cron: String) {
        let fields = cron.components(separatedBy: " ")
        guard fields.count == 5 else {
            schedulePreset = .custom
            return
        }

        let f0 = fields[0]  // minute (or step)
        let f1 = fields[1]  // hour (or step)
        let f2 = fields[2]  // day-of-month
        let f3 = fields[3]  // month
        let f4 = fields[4]  // day-of-week

        // ── Fixed-interval presets (exact string matches) ──────────────────

        if cron == "*/5 * * * *"  { schedulePreset = .every5Min;  return }
        if cron == "*/10 * * * *" { schedulePreset = .every10Min; return }
        if cron == "*/15 * * * *" { schedulePreset = .every15Min; return }
        if cron == "*/30 * * * *" { schedulePreset = .every30Min; return }
        if cron == "0 * * * *"    { schedulePreset = .everyHour;  return }
        if cron == "0 */2 * * *"  { schedulePreset = .every2Hours; return }
        if cron == "0 */4 * * *"  { schedulePreset = .every4Hours; return }

        // ── Patterns that require numeric minute / hour fields ──────────────

        guard let minute = Int(f0), let hour = Int(f1) else {
            schedulePreset = .custom
            return
        }

        // weekdaysAt: "{int} {int} * * 1-5"
        if f2 == "*" && f3 == "*" && f4 == "1-5" {
            schedulePreset = .weekdaysAt
            selectedMinute = minute
            selectedHour = hour
            return
        }

        // weeklyOn: "{int} {int} * * {comma-separated weekday ints}"
        if f2 == "*" && f3 == "*" && f4 != "*" {
            let tokens = f4.components(separatedBy: ",")
            let parsed = tokens.compactMap { Int($0) }
            if parsed.count == tokens.count && parsed.allSatisfy({ (0...6).contains($0) }) {
                schedulePreset = .weeklyOn
                selectedMinute = minute
                selectedHour = hour
                selectedWeekdays = Set(parsed)
                return
            }
        }

        // monthlyOn: "{int} {int} {int} * *"
        if let dom = Int(f2), f3 == "*" && f4 == "*" {
            schedulePreset = .monthlyOn
            selectedMinute = minute
            selectedHour = hour
            selectedDayOfMonth = dom
            return
        }

        // dailyAt: "{int} {int} * * *"
        if f2 == "*" && f3 == "*" && f4 == "*" {
            schedulePreset = .dailyAt
            selectedMinute = minute
            selectedHour = hour
            return
        }

        // Anything else
        schedulePreset = .custom
    }
}
