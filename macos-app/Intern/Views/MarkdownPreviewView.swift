import SwiftUI

struct MarkdownPreviewView: View {
    let text: String

    private var attributedText: AttributedString {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return AttributedString()
        }

        do {
            var result = try AttributedString(
                markdown: text,
                options: AttributedString.MarkdownParsingOptions(
                    interpretedSyntax: .inlineOnlyPreservingWhitespace
                )
            )

            // Apply style overrides for design token consistency
            for run in result.runs {
                let range = run.range

                // Headings
                if let intent = run.presentationIntent {
                    for component in intent.components {
                        if case .header(let level) = component.kind {
                            if level == 1 {
                                result[range].font = .system(size: 20, weight: .bold)
                            } else if level == 2 {
                                result[range].font = .system(size: 18, weight: .bold)
                            } else {
                                result[range].font = .system(size: 13.5, weight: .semibold)
                            }
                            result[range].foregroundColor = Color.inTextPrimary
                        }
                    }
                }

                // Inline code
                if let inlineIntent = run.inlinePresentationIntent, inlineIntent.contains(.code) {
                    result[range].font = .system(size: 11, design: .monospaced)
                    result[range].foregroundColor = Color.inAccentLight
                }

                // Links
                if run.link != nil {
                    result[range].foregroundColor = Color.inAccentLight
                }
            }

            // Set default styling for body text
            var container = AttributeContainer()
            container.font = .system(size: 13)
            container.foregroundColor = Color.inTextPrimary
            result.mergeAttributes(container, mergePolicy: .keepCurrent)

            return result
        } catch {
            // Fallback to plain text on parse failure
            var plain = AttributedString(text)
            plain.font = .system(size: 13)
            plain.foregroundColor = Color.inTextPrimary
            return plain
        }
    }

    var body: some View {
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            Text("Nothing to preview.")
                .font(.inCaption)
                .foregroundColor(.inTextMuted)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.inCodeBackground)
        } else {
            ScrollView(.vertical, showsIndicators: true) {
                Text(attributedText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .lineSpacing(4)
                    .padding(12)
            }
            .background(Color.inCodeBackground)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}
