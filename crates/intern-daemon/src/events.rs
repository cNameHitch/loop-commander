//! Event broadcasting and serialization for the Intern daemon.
//!
//! The `DaemonEvent` enum is defined in `intern-core`. This module provides
//! helper functions for serializing events into JSON-RPC notifications
//! suitable for pushing over the Unix socket event stream.

use intern_core::DaemonEvent;

/// Serialize a `DaemonEvent` into a JSON-RPC 2.0 notification string.
///
/// The format follows JSON-RPC notification conventions (no `id` field):
/// ```json
/// {"jsonrpc":"2.0","method":"event","params":{...}}
/// ```
///
/// A trailing newline is appended so the client can use line-delimited
/// parsing on the event stream.
///
/// # Examples
///
/// ```
/// use intern_core::DaemonEvent;
/// use intern_daemon::events::event_to_notification;
///
/// let event = DaemonEvent::TaskStarted {
///     task_id: "lc-abcd1234".into(),
///     task_name: "My Task".into(),
/// };
/// let notification = event_to_notification(&event);
/// assert!(notification.contains("\"method\":\"event\""));
/// assert!(notification.ends_with('\n'));
/// ```
pub fn event_to_notification(event: &DaemonEvent) -> String {
    let params = serde_json::to_value(event).unwrap_or(serde_json::Value::Null);
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "event",
        "params": params,
    });
    let mut s = serde_json::to_string(&notification).unwrap_or_default();
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_to_notification_task_started() {
        let event = DaemonEvent::TaskStarted {
            task_id: "lc-abcd1234".into(),
            task_name: "Test Task".into(),
        };
        let notification = event_to_notification(&event);

        assert!(notification.ends_with('\n'));
        assert!(notification.contains("\"jsonrpc\":\"2.0\""));
        assert!(notification.contains("\"method\":\"event\""));
        assert!(notification.contains("\"params\""));
        assert!(notification.contains("lc-abcd1234"));
        assert!(notification.contains("Test Task"));
    }

    #[test]
    fn event_to_notification_task_completed() {
        let event = DaemonEvent::TaskCompleted {
            task_id: "lc-12345678".into(),
            task_name: "Build".into(),
            duration_secs: 42,
            cost_usd: Some(1.23),
        };
        let notification = event_to_notification(&event);

        assert!(notification.contains("\"method\":\"event\""));
        assert!(notification.contains("42"));
        assert!(notification.contains("1.23"));
    }

    #[test]
    fn event_to_notification_task_failed() {
        let event = DaemonEvent::TaskFailed {
            task_id: "lc-deadbeef".into(),
            task_name: "Deploy".into(),
            exit_code: 1,
            summary: "Process crashed".into(),
        };
        let notification = event_to_notification(&event);

        assert!(notification.contains("\"method\":\"event\""));
        assert!(notification.contains("Process crashed"));
    }

    #[test]
    fn event_to_notification_status_changed() {
        let event = DaemonEvent::TaskStatusChanged {
            task_id: "lc-11111111".into(),
            old_status: "active".into(),
            new_status: "paused".into(),
        };
        let notification = event_to_notification(&event);

        assert!(notification.contains("active"));
        assert!(notification.contains("paused"));
    }

    #[test]
    fn event_to_notification_health_repair() {
        let event = DaemonEvent::HealthRepair {
            task_id: "lc-22222222".into(),
            action: "re-registered plist".into(),
        };
        let notification = event_to_notification(&event);

        assert!(notification.contains("re-registered plist"));
    }

    #[test]
    fn event_to_notification_budget_exceeded() {
        let event = DaemonEvent::BudgetExceeded {
            task_id: "lc-33333333".into(),
            task_name: "Expensive Task".into(),
            daily_spend: 45.0,
            cap: 40.0,
        };
        let notification = event_to_notification(&event);

        assert!(notification.contains("45"));
        assert!(notification.contains("40"));
    }

    #[test]
    fn event_serialization_roundtrip() {
        let event = DaemonEvent::TaskStarted {
            task_id: "lc-roundtrp".into(),
            task_name: "Roundtrip".into(),
        };
        let notification = event_to_notification(&event);

        // Parse the notification as JSON to verify structure.
        let trimmed = notification.trim();
        let parsed: serde_json::Value = serde_json::from_str(trimmed).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "event");
        assert!(parsed.get("id").is_none());

        // Deserialize the params back into a DaemonEvent.
        let params = &parsed["params"];
        let deserialized: DaemonEvent = serde_json::from_value(params.clone()).unwrap();

        match deserialized {
            DaemonEvent::TaskStarted { task_id, task_name } => {
                assert_eq!(task_id, "lc-roundtrp");
                assert_eq!(task_name, "Roundtrip");
            }
            _ => panic!("Expected TaskStarted variant"),
        }
    }
}
