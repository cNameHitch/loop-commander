import Foundation

private let _iso8601Full: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f
}()

private let _iso8601Basic: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime]
    return f
}()

private let _decimalFormatter: NumberFormatter = {
    let f = NumberFormatter()
    f.numberStyle = .decimal
    return f
}()

/// Relative time string from a Date.
/// JSX reference: relativeTime() -- "just now", "5m ago", "2h ago", "3d ago"
func relativeTime(_ date: Date) -> String {
    let diff = Date().timeIntervalSince(date)
    let mins = Int(diff / 60)
    if mins < 1 { return "just now" }
    if mins < 60 { return "\(mins)m ago" }
    let hrs = mins / 60
    if hrs < 24 { return "\(hrs)h ago" }
    let days = hrs / 24
    return "\(days)d ago"
}

/// Relative time string from an ISO 8601 date string.
func relativeTime(_ isoString: String) -> String {
    if let date = _iso8601Full.date(from: isoString) {
        return relativeTime(date)
    }
    if let date = _iso8601Basic.date(from: isoString) {
        return relativeTime(date)
    }
    return isoString
}

/// Format duration in seconds to human-readable string.
/// JSX reference: formatDuration() -- "45s", "5m 12s", "5m"
func formatDuration(_ secs: Int) -> String {
    if secs < 60 {
        return "\(secs)s"
    }
    let mins = secs / 60
    let remainSecs = secs % 60
    if remainSecs == 0 {
        return "\(mins)m"
    }
    return "\(mins)m \(remainSecs)s"
}

/// Format a timestamp for display in log entries.
func formatTimestamp(_ isoString: String) -> String {
    var date = _iso8601Full.date(from: isoString)
    if date == nil {
        date = _iso8601Basic.date(from: isoString)
    }

    guard let d = date else { return isoString }

    let displayFormatter = DateFormatter()
    displayFormatter.dateFormat = "MMM d, HH:mm:ss"
    return displayFormatter.string(from: d)
}

/// Format a token count with commas.
func formatTokens(_ count: Int?) -> String {
    guard let count = count else { return "0" }
    return _decimalFormatter.string(from: NSNumber(value: count)) ?? "\(count)"
}

/// Format a cost as dollar string.
func formatCost(_ cost: Double?) -> String {
    guard let cost = cost else { return "$0.00" }
    return String(format: "$%.2f", cost)
}

/// Format uptime from seconds to human-readable.
func formatUptime(_ seconds: Int) -> String {
    let days = seconds / 86400
    let hours = (seconds % 86400) / 3600
    if days > 0 {
        return "\(days)d \(hours)h"
    }
    let mins = (seconds % 3600) / 60
    if hours > 0 {
        return "\(hours)h \(mins)m"
    }
    return "\(mins)m"
}

/// Contract the home directory prefix to `~` for display.
func abbreviatePath(_ path: String) -> String {
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    if path.hasPrefix(home) {
        return "~" + path.dropFirst(home.count)
    }
    return path
}

/// Parse an ISO 8601 date string to Date.
func parseISO8601(_ string: String) -> Date? {
    if let date = _iso8601Full.date(from: string) {
        return date
    }
    return _iso8601Basic.date(from: string)
}
