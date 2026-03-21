// swift-tools-version: 5.9
import PackageDescription

// NOTE: GAP-04 / GAP-05 / GAP-06 — Test target constraints
//
// Two blockers currently prevent `swift test` from passing:
//
// BLOCKER 1 — SPM executable dependency restriction
//   Swift Package Manager does not allow a testTarget to declare a dependency
//   on an executableTarget.  The fix is to refactor the project into:
//     - A `.target(name: "InternLib")` library containing all
//       application source (with the public API surface marked `public`).
//     - A thin `.executableTarget(name: "Intern")` that contains only
//       the @main entry point (InternApp.swift) and depends on
//       InternLib.
//     - The `.testTarget(name: "InternTests")` depending on
//       InternLib and using `@testable import InternLib`.
//   The test files in Tests/InternTests/ are fully written and will
//   compile without modification once this refactor is complete.  The local
//   stub types in PromptOptimizerViewModelTests.swift replace the real types
//   during the interim and are removed after @testable import is restored.
//
// BLOCKER 2 — XCTest requires Xcode (not available with Command Line Tools)
//   `swift test` will additionally fail with "no such module 'XCTest'" on
//   machines that have only the Xcode Command Line Tools installed.  XCTest
//   is bundled with the full Xcode IDE.  Install Xcode to resolve this.
//
// When both blockers are resolved, `swift test` from macos-app/ will run
// the PromptOptimizerViewModelTests suite directly.

let package = Package(
    name: "Intern",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "Intern", targets: ["Intern"]),
    ],
    targets: [
        .executableTarget(
            name: "Intern",
            path: "Intern"
        ),

        // Test infrastructure for GAP-04/05/06.
        // No dependency on Intern is declared here because SPM
        // forbids testTargets from depending on executableTargets.
        // The test files contain self-contained stubs of the types under
        // test; they are ready to be wired to the real implementation once
        // the InternLib library target is introduced.
        .testTarget(
            name: "InternTests",
            path: "Tests/InternTests"
        ),
    ]
)
