import Foundation

/// Schedule type mirroring the Rust `Schedule` tagged enum.
/// Rust serializes with `#[serde(tag = "type", rename_all = "snake_case")]`.
enum Schedule: Codable, Hashable {
    case cron(expression: String)
    case interval(seconds: Int)
    case calendar(minute: Int?, hour: Int?, day: Int?, weekday: Int?, month: Int?)

    enum CodingKeys: String, CodingKey {
        case type
        case expression
        case seconds
        case minute
        case hour
        case day
        case weekday
        case month
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "cron":
            let expression = try container.decode(String.self, forKey: .expression)
            self = .cron(expression: expression)
        case "interval":
            let seconds = try container.decode(Int.self, forKey: .seconds)
            self = .interval(seconds: seconds)
        case "calendar":
            let minute = try container.decodeIfPresent(Int.self, forKey: .minute)
            let hour = try container.decodeIfPresent(Int.self, forKey: .hour)
            let day = try container.decodeIfPresent(Int.self, forKey: .day)
            let weekday = try container.decodeIfPresent(Int.self, forKey: .weekday)
            let month = try container.decodeIfPresent(Int.self, forKey: .month)
            self = .calendar(minute: minute, hour: hour, day: day, weekday: weekday, month: month)
        default:
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: container.codingPath,
                    debugDescription: "Unknown schedule type: \(type)"
                )
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .cron(let expression):
            try container.encode("cron", forKey: .type)
            try container.encode(expression, forKey: .expression)
        case .interval(let seconds):
            try container.encode("interval", forKey: .type)
            try container.encode(seconds, forKey: .seconds)
        case .calendar(let minute, let hour, let day, let weekday, let month):
            try container.encode("calendar", forKey: .type)
            try container.encodeIfPresent(minute, forKey: .minute)
            try container.encodeIfPresent(hour, forKey: .hour)
            try container.encodeIfPresent(day, forKey: .day)
            try container.encodeIfPresent(weekday, forKey: .weekday)
            try container.encodeIfPresent(month, forKey: .month)
        }
    }

    /// Human-readable description
    var humanDescription: String {
        switch self {
        case .cron(let expression):
            return "Cron: \(expression)"
        case .interval(let seconds):
            if seconds < 60 {
                return "Every \(seconds)s"
            } else if seconds < 3600 {
                return "Every \(seconds / 60)m"
            } else {
                return "Every \(seconds / 3600)h"
            }
        case .calendar(let minute, let hour, _, let weekday, _):
            let time: String
            if let h = hour, let m = minute {
                time = String(format: "%02d:%02d", h, m)
            } else if let h = hour {
                time = String(format: "%02d:00", h)
            } else {
                time = "every interval"
            }
            if let d = weekday {
                let dayNames = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                let name = (0...6).contains(d) ? dayNames[d] : "?"
                return "\(name)s at \(time)"
            }
            return "Daily at \(time)"
        }
    }

    /// Cron expression string (if cron type)
    var cronExpression: String? {
        if case .cron(let expr) = self {
            return expr
        }
        return nil
    }
}
