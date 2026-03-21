import Foundation
import UserNotifications
import AppKit

/// Sends native macOS notifications for task events.
/// Requires the app bundle to be code-signed for UNUserNotification to work.
@MainActor
final class NotificationManager: NSObject, ObservableObject {

    var daemonClient: DaemonClient?
    private var authorized = false

    override init() {
        super.init()
    }

    func setup() {
        let center = UNUserNotificationCenter.current()
        center.delegate = self
        registerCategories()

        Task.detached {
            do {
                let granted = try await center.requestAuthorization(options: [.alert, .sound])
                await MainActor.run { self.authorized = granted }
            } catch {
                await MainActor.run { self.authorized = false }
            }
        }
    }

    private func registerCategories() {
        let viewLogs = UNNotificationAction(identifier: "LC_VIEW_LOGS", title: "See What Happened", options: [.foreground])
        let runAgain = UNNotificationAction(identifier: "LC_RUN_AGAIN", title: "Try Again", options: [.foreground])

        let success = UNNotificationCategory(identifier: "LC_SUCCESS", actions: [viewLogs], intentIdentifiers: [])
        let failure = UNNotificationCategory(identifier: "LC_FAILURE", actions: [viewLogs, runAgain], intentIdentifiers: [])
        let timeout = UNNotificationCategory(identifier: "LC_TIMEOUT", actions: [viewLogs], intentIdentifiers: [])

        UNUserNotificationCenter.current().setNotificationCategories([success, failure, timeout])
    }

    func sendTaskSuccess(taskId: String, taskName: String, durationSecs: Int, costUsd: Double?) {
        let duration = durationSecs > 0 ? formatDuration(durationSecs) : "just now"
        var body: String
        if let cost = costUsd, cost > 0 {
            body = "Finished in \(duration) — cost \(formatCost(cost))"
        } else {
            body = "Finished in \(duration)."
        }

        let content = UNMutableNotificationContent()
        content.title = "Done"
        content.subtitle = taskName
        content.body = body
        content.categoryIdentifier = "LC_SUCCESS"
        content.threadIdentifier = "lc.\(taskId)"
        deliver(id: "lc-s-\(taskId)-\(Int(Date().timeIntervalSince1970))", content: content)
    }

    func sendTaskFailure(taskId: String, taskName: String, exitCode: Int, summary: String) {
        let content = UNMutableNotificationContent()
        content.title = "Something went wrong"
        content.subtitle = taskName
        content.body = String(summary.prefix(120))
        content.sound = .default
        content.categoryIdentifier = "LC_FAILURE"
        content.threadIdentifier = "lc.\(taskId)"
        content.userInfo = ["taskId": taskId]
        deliver(id: "lc-f-\(taskId)-\(Int(Date().timeIntervalSince1970))", content: content)
    }

    func sendTaskTimeout(taskId: String, taskName: String, summary: String) {
        let content = UNMutableNotificationContent()
        content.title = "Ran out of time"
        content.subtitle = taskName
        content.body = String(summary.prefix(120))
        content.sound = .default
        content.categoryIdentifier = "LC_TIMEOUT"
        content.threadIdentifier = "lc.\(taskId)"
        content.userInfo = ["taskId": taskId]
        deliver(id: "lc-t-\(taskId)-\(Int(Date().timeIntervalSince1970))", content: content)
    }

    private func deliver(id: String, content: UNMutableNotificationContent) {
        let request = UNNotificationRequest(identifier: id, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request) { _ in }
    }
}

// MARK: - UNUserNotificationCenterDelegate

extension NotificationManager: UNUserNotificationCenterDelegate {
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let userInfo = response.notification.request.content.userInfo
        let taskId = userInfo["taskId"] as? String

        Task { @MainActor in
            NSApp.activate(ignoringOtherApps: true)

            switch response.actionIdentifier {
            case "LC_RUN_AGAIN":
                if let taskId = taskId {
                    try? await self.daemonClient?.runTaskNow(taskId)
                }
            default:
                NotificationCenter.default.post(name: .switchToLogs, object: nil)
            }
        }
        completionHandler()
    }
}
