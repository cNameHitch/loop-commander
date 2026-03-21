import SwiftUI

/// A discovered Claude Code command from a .claude/commands/ directory.
struct ClaudeCommand: Identifiable, Hashable {
    let id = UUID()
    let name: String
    let description: String
    let content: String
    let projectPath: String
    let projectName: String
    let filePath: String
}

/// Scans the filesystem for Claude Code custom commands and presents them for import.
struct CommandImportView: View {
    let onImport: (ClaudeCommand) -> Void
    let onDismiss: () -> Void

    @State private var commands: [ClaudeCommand] = []
    @State private var isScanning = true
    @State private var searchText = ""
    @State private var selectedCommand: ClaudeCommand?

    private var filteredCommands: [ClaudeCommand] {
        if searchText.isEmpty { return commands }
        let query = searchText.lowercased()
        return commands.filter {
            $0.name.lowercased().contains(query) ||
            $0.description.lowercased().contains(query) ||
            $0.projectName.lowercased().contains(query)
        }
    }

    /// Group commands by project
    private var groupedCommands: [(String, [ClaudeCommand])] {
        let grouped = Dictionary(grouping: filteredCommands) { $0.projectName }
        return grouped.sorted { $0.key < $1.key }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            HStack {
                Text("Import Claude Command")
                    .font(.inHeading)
                    .foregroundColor(.inTextPrimary)
                Spacer()
                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                        .font(.system(size: 14))
                        .foregroundColor(.inTextMuted)
                }
                .buttonStyle(.plain)
            }
            .padding(.bottom, 16)

            // Search
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12))
                    .foregroundColor(.inTextMuted)
                TextField("Filter commands...", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.inInput)
                    .foregroundColor(.inTextPrimary)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
            .background(Color.inCodeBackground)
            .overlay(
                RoundedRectangle(cornerRadius: INRadius.button)
                    .stroke(Color.inBorderInput, lineWidth: 1)
            )
            .cornerRadius(INRadius.button)
            .padding(.bottom, 8)

            // Rescan button
            HStack {
                Spacer()
                Button {
                    isScanning = true
                    Task { await scanForCommands() }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "arrow.clockwise")
                            .font(.system(size: 11))
                        Text("Rescan")
                    }
                }
                .buttonStyle(INToolbarButtonStyle())
                .disabled(isScanning)
            }
            .padding(.bottom, 8)

            // Content
            if isScanning {
                VStack(spacing: 12) {
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Scanning for Claude commands...")
                        .font(.inBodyMedium)
                        .foregroundColor(.inTextMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if commands.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "doc.text.magnifyingglass")
                        .font(.system(size: 40))
                        .foregroundColor(.inTextFaint)
                    Text("No commands found")
                        .font(.inBodyMedium)
                        .foregroundColor(.inTextMuted)
                    Text("Place .md files in .claude/commands/ within your projects")
                        .font(.inCaption)
                        .foregroundColor(.inTextSubtle)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 16) {
                        ForEach(groupedCommands, id: \.0) { projectName, cmds in
                            VStack(alignment: .leading, spacing: 8) {
                                // Project header
                                HStack(spacing: 6) {
                                    Image(systemName: "folder.fill")
                                        .font(.system(size: 10))
                                        .foregroundColor(.inAccent)
                                    Text(projectName)
                                        .font(.inSectionLabel)
                                        .foregroundColor(.inTextMuted)
                                }

                                ForEach(cmds) { cmd in
                                    CommandRow(
                                        command: cmd,
                                        isSelected: selectedCommand == cmd
                                    )
                                    .onTapGesture { selectedCommand = cmd }
                                }
                            }
                        }
                    }
                }
            }

            Spacer(minLength: 20)

            // Footer
            HStack(spacing: 10) {
                if let cmd = selectedCommand {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Selected: /\(cmd.name)")
                            .font(.inCaption)
                            .foregroundColor(.inAccentLight)
                        Text(cmd.projectName)
                            .font(.system(size: 10))
                            .foregroundColor(.inTextSubtle)
                    }
                }
                Spacer()
                Button("Cancel", action: onDismiss)
                    .buttonStyle(INSecondaryButtonStyle())
                Button("Import") {
                    if let cmd = selectedCommand {
                        onImport(cmd)
                        onDismiss()
                    }
                }
                .buttonStyle(INPrimaryButtonStyle())
                .disabled(selectedCommand == nil)
            }
        }
        .padding(32)
        .frame(width: 600, height: 500)
        .background(Color.inSurface)
        .task { await scanForCommands() }
    }

    private func scanForCommands() async {
        let found = await Task.detached {
            CommandScanner.scan()
        }.value
        commands = found
        isScanning = false
    }
}

// MARK: - Command Row

private struct CommandRow: View {
    let command: ClaudeCommand
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text("/\(command.name)")
                    .font(.inBodyMedium)
                    .foregroundColor(isSelected ? .inAccentLight : .inTextPrimary)
                if !command.description.isEmpty {
                    Text(command.description)
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                        .lineLimit(2)
                }
            }
            Spacer()
            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 16))
                .foregroundColor(isSelected ? .inAccent : .inTextFaint)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .background(isSelected ? Color.inAccentBgSubtle : Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(isSelected ? Color.inAccent : Color.inBorderInput, lineWidth: 1)
        )
        .cornerRadius(INRadius.button)
        .contentShape(Rectangle())
    }
}

// MARK: - Filesystem Scanner

enum CommandScanner {
    /// Scan well-known locations for user-authored Claude Code commands and skills.
    static func scan() -> [ClaudeCommand] {
        var results: [ClaudeCommand] = []
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser

        // 1. Global user commands: ~/.claude/commands/
        let globalCmds = home.appendingPathComponent(".claude/commands")
        results.append(contentsOf: scanDirectory(globalCmds, projectName: "Global", projectPath: globalCmds.path))

        // 2. Scan common project roots for .claude/{commands,skills}
        let searchRoots = [
            home.appendingPathComponent("Desktop/git"),
            home.appendingPathComponent("Developer"),
            home.appendingPathComponent("Projects"),
            home.appendingPathComponent("Documents"),
            home.appendingPathComponent("repos"),
            home.appendingPathComponent("src"),
            home.appendingPathComponent("Code"),
        ]

        for root in searchRoots {
            guard fm.fileExists(atPath: root.path) else { continue }
            guard let entries = try? fm.contentsOfDirectory(
                at: root,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: []
            ) else { continue }

            for entry in entries {
                // Skip hidden entries (but NOT .claude itself at project level)
                let name = entry.lastPathComponent
                if name.hasPrefix(".") { continue }

                let projectName = name
                let claudeBase = entry.appendingPathComponent(".claude")
                guard fm.fileExists(atPath: claudeBase.path) else { continue }

                // .claude/commands/*.md
                let cmdsDir = claudeBase.appendingPathComponent("commands")
                results.append(contentsOf: scanDirectory(cmdsDir, projectName: projectName, projectPath: entry.path))

                // .claude/skills/*/SKILL.md (each skill is a subdirectory)
                let skillsDir = claudeBase.appendingPathComponent("skills")
                results.append(contentsOf: scanSkillsDirectory(skillsDir, projectName: projectName, projectPath: entry.path))
            }
        }

        return results.sorted { $0.name < $1.name }
    }

    /// Parse all .md files in a single flat directory (e.g. commands/).
    private static func scanDirectory(_ dir: URL, projectName: String, projectPath: String) -> [ClaudeCommand] {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil) else {
            return []
        }

        return files.compactMap { url -> ClaudeCommand? in
            guard url.pathExtension == "md" else { return nil }
            guard let content = try? String(contentsOf: url, encoding: .utf8) else { return nil }

            let name = parseFrontmatterField(content, field: "name")
                ?? url.deletingPathExtension().lastPathComponent
            let description = parseFrontmatterDescription(content)

            return ClaudeCommand(
                name: name,
                description: description,
                content: content,
                projectPath: projectPath,
                projectName: projectName,
                filePath: url.path
            )
        }
    }

    /// Scan .claude/skills/ where each skill is a subdirectory containing SKILL.md.
    private static func scanSkillsDirectory(_ dir: URL, projectName: String, projectPath: String) -> [ClaudeCommand] {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else { return [] }

        return entries.compactMap { subdir -> ClaudeCommand? in
            let skillFile = subdir.appendingPathComponent("SKILL.md")
            guard let content = try? String(contentsOf: skillFile, encoding: .utf8) else { return nil }

            let name = parseFrontmatterField(content, field: "name")
                ?? subdir.lastPathComponent
            let description = parseFrontmatterDescription(content)

            return ClaudeCommand(
                name: name,
                description: description,
                content: content,
                projectPath: projectPath,
                projectName: projectName,
                filePath: skillFile.path
            )
        }
    }

    /// Extract the `description:` field from YAML frontmatter.
    private static func parseFrontmatterDescription(_ content: String) -> String {
        return parseFrontmatterField(content, field: "description") ?? ""
    }

    /// Extract a named field from YAML frontmatter.
    private static func parseFrontmatterField(_ content: String, field: String) -> String? {
        let lines = content.components(separatedBy: .newlines)
        guard lines.first?.trimmingCharacters(in: .whitespaces) == "---" else { return nil }

        let prefix = "\(field):"
        for (i, line) in lines.enumerated() {
            if i == 0 { continue }
            if line.trimmingCharacters(in: .whitespaces) == "---" { break }
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix(prefix) {
                let value = trimmed
                    .dropFirst(prefix.count)
                    .trimmingCharacters(in: .whitespaces)
                    .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
                return value.isEmpty ? nil : value
            }
        }
        return nil
    }
}
