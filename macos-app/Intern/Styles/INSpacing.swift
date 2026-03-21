import SwiftUI

enum INSpacing {
    /// 3px - Badge internal padding vertical, tag padding vertical
    static let xxxs: CGFloat = 3
    /// 4px - Tag gap, tag list gap, close button padding
    static let xxs: CGFloat  = 4
    /// 5px - Badge icon-label gap, new-task button icon gap
    static let xs: CGFloat   = 5
    /// 6px - Metric label bottom margin, two-col gap small,
    ///        nav button gap, divider margin
    static let sm: CGFloat   = 6
    /// 8px - Log grid gap, tag input gap, log search gap,
    ///        metric label bottom margin, tag list top margin,
    ///        search input padding vertical
    static let md: CGFloat   = 8
    /// 10px - Badge padding horizontal, tag padding horizontal,
    ///         status icon padding, row border spacing,
    ///         log entry row padding vertical, footer button gap
    static let lg: CGFloat   = 10
    /// 12px - Metrics grid gap, two-col grid gap, detail-info grid gap,
    ///         detail task heading - badge gap, back-button gap,
    ///         code block padding, filter button padding horizontal
    static let xl: CGFloat   = 12
    /// 14px - Task row padding vertical, logo-title gap,
    ///         log expanded bottom padding, code block padding (larger),
    ///         search input padding horizontal
    static let xxl: CGFloat  = 14
    /// 16px - Header padding vertical, log row padding horizontal,
    ///         log header padding, detail section gap, nav button padding,
    ///         two-col gap (larger), back-button padding
    static let xxxl: CGFloat = 16
    /// 18px - Metric card padding top
    static let p18: CGFloat  = 18
    /// 20px - Metric card padding horizontal, task row padding horizontal,
    ///         detail info grid gap (row), column header padding horizontal,
    ///         detail section padding, detail info gap
    static let p20: CGFloat  = 20
    /// 24px - Detail info card padding, footer button padding horizontal (save)
    static let p24: CGFloat  = 24
    /// 28px - Header padding horizontal, metrics bar outer padding,
    ///         content area outer padding, editor title bottom margin,
    ///         editor footer top margin
    static let p28: CGFloat  = 28
    /// 32px - Editor modal padding, empty state padding
    static let p32: CGFloat  = 32
    /// 46px - Log expanded content left inset (22px icon col + 16px row pad + 8px gap)
    static let logExpandedInset: CGFloat = 46
}
