import Foundation

enum SchedulePreset: String, CaseIterable, Identifiable {
    case every5Min    = "every_5_min"
    case every10Min   = "every_10_min"
    case every15Min   = "every_15_min"
    case every30Min   = "every_30_min"
    case everyHour    = "every_hour"
    case every2Hours  = "every_2_hours"
    case every4Hours  = "every_4_hours"
    case dailyAt      = "daily_at"
    case weekdaysAt   = "weekdays_at"
    case weeklyOn     = "weekly_on"
    case monthlyOn    = "monthly_on"
    case custom       = "custom"

    var id: String { rawValue }

    // MARK: - Display Name

    var displayName: String {
        switch self {
        case .every5Min:   return "Every 5 minutes"
        case .every10Min:  return "Every 10 minutes"
        case .every15Min:  return "Every 15 minutes"
        case .every30Min:  return "Every 30 minutes"
        case .everyHour:   return "Every hour"
        case .every2Hours: return "Every 2 hours"
        case .every4Hours: return "Every 4 hours"
        case .dailyAt:     return "Daily at..."
        case .weekdaysAt:  return "Weekdays at..."
        case .weeklyOn:    return "Weekly on..."
        case .monthlyOn:   return "Monthly on..."
        case .custom:      return "Custom (Advanced)"
        }
    }

    // MARK: - Picker Requirements

    var requiresTimePicker: Bool {
        switch self {
        case .dailyAt, .weekdaysAt, .weeklyOn, .monthlyOn:
            return true
        default:
            return false
        }
    }

    var requiresDayOfWeekPicker: Bool {
        self == .weeklyOn
    }

    var requiresDayOfMonthPicker: Bool {
        self == .monthlyOn
    }

    var isCustom: Bool {
        self == .custom
    }

    // MARK: - Cron Expression

    func cronExpression(hour: Int, minute: Int, weekdays: Set<Int>, dayOfMonth: Int) -> String {
        switch self {
        case .every5Min:   return "*/5 * * * *"
        case .every10Min:  return "*/10 * * * *"
        case .every15Min:  return "*/15 * * * *"
        case .every30Min:  return "*/30 * * * *"
        case .everyHour:   return "0 * * * *"
        case .every2Hours: return "0 */2 * * *"
        case .every4Hours: return "0 */4 * * *"
        case .dailyAt:     return "\(minute) \(hour) * * *"
        case .weekdaysAt:  return "\(minute) \(hour) * * 1-5"
        case .weeklyOn:
            let dayList = weekdays.isEmpty
                ? "1"
                : weekdays.sorted().map(String.init).joined(separator: ",")
            return "\(minute) \(hour) * * \(dayList)"
        case .monthlyOn:   return "\(minute) \(hour) \(dayOfMonth) * *"
        case .custom:      return ""
        }
    }

    // MARK: - Human Description

    func humanDescription(hour: Int, minute: Int, weekdays: Set<Int>, dayOfMonth: Int) -> String {
        switch self {
        case .every5Min:   return "Every 5 minutes"
        case .every10Min:  return "Every 10 minutes"
        case .every15Min:  return "Every 15 minutes"
        case .every30Min:  return "Every 30 minutes"
        case .everyHour:   return "Every hour"
        case .every2Hours: return "Every 2 hours"
        case .every4Hours: return "Every 4 hours"
        case .dailyAt:
            let timeString = String(format: "%02d:%02d", hour, minute)
            return "Every day at \(timeString)"
        case .weekdaysAt:
            let timeString = String(format: "%02d:%02d", hour, minute)
            return "Every weekday at \(timeString)"
        case .weeklyOn:
            let dayNames = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
            let timeString = String(format: "%02d:%02d", hour, minute)
            let dayNameList = weekdays.sorted()
                .compactMap { (0...6).contains($0) ? dayNames[$0] : nil }
                .joined(separator: ", ")
            return "Weekly on \(dayNameList) at \(timeString)"
        case .monthlyOn:
            let timeString = String(format: "%02d:%02d", hour, minute)
            let ordinal: String
            switch dayOfMonth {
            case 1, 21: ordinal = "st"
            case 2, 22: ordinal = "nd"
            case 3, 23: ordinal = "rd"
            default:    ordinal = "th"
            }
            return "Monthly on the \(dayOfMonth)\(ordinal) at \(timeString)"
        case .custom:
            return "Custom schedule"
        }
    }
}
