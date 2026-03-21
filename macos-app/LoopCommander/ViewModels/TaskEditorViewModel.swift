import Foundation

/// ViewModel for the task editor sheet.
@MainActor
class TaskEditorViewModel: ObservableObject {
    @Published var draft: LCTaskDraft
    @Published var templates: [TaskTemplate] = []
    @Published var selectedTemplate: String? = nil
    @Published var isSaving: Bool = false
    @Published var error: String?
    @Published var validationErrors: [String] = []
    @Published var promptGeneratorVM = PromptGeneratorViewModel()

    let isNew: Bool
    let taskId: String?
    private var client: DaemonClient?

    init(isNew: Bool, task: LCTask? = nil) {
        self.isNew = isNew
        self.taskId = task?.id
        if let task = task {
            self.draft = LCTaskDraft(from: task)
        } else {
            self.draft = LCTaskDraft()
        }
    }

    /// Initialize with an imported Claude Code command.
    init(isNew: Bool, importedCommand: ClaudeCommand) {
        self.isNew = true
        self.taskId = nil
        self.draft = LCTaskDraft(from: importedCommand)
    }

    func setClient(_ client: DaemonClient) {
        self.client = client
        promptGeneratorVM.setClient(client)
    }

    func loadTemplates() async {
        guard let client = client, isNew else { return }
        do {
            templates = try await client.getTemplates()
        } catch {
            // Templates are optional
        }
    }

    func applyTemplate(_ template: TaskTemplate) {
        draft = LCTaskDraft(from: template)
        selectedTemplate = template.slug
    }

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
        if draft.command.count > 10_000 {
            validationErrors.append("Command must be 10,000 characters or fewer")
        }
        if draft.maxBudget <= 0 {
            validationErrors.append("Budget must be greater than 0")
        }
        if draft.maxBudget > 100 {
            validationErrors.append("Budget must be $100 or less")
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

    func save() async -> Bool {
        guard validate() else { return false }
        guard let client = client else { return false }

        isSaving = true
        error = nil

        do {
            if isNew {
                let params = draft.toCreateInput()
                _ = try await client.createTask(params)
            } else if let id = taskId {
                let params = draft.toUpdateInput(id: id)
                _ = try await client.updateTask(params)
            }
            isSaving = false
            return true
        } catch {
            self.error = error.localizedDescription
            isSaving = false
            return false
        }
    }
}
