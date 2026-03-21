import SwiftUI

/// Reusable agent picker used inside PromptGeneratorPanel.
///
/// Renders a search field, a scrollable list grouped by category, and a
/// FlowLayout of removable chips for the current selection.  All design
/// tokens are sourced directly from the existing Color+Intern (unchanged),
/// Font+Intern (unchanged), and INRadius files.
struct AgentPickerView: View {

    let agents: [AgentEntry]
    @Binding var selectedAgents: Set<String>
    let agentCategories: [String]

    @State private var searchText: String = ""
    @FocusState private var searchFocused: Bool

    // MARK: - Filtering

    private var filteredAgents: [AgentEntry] {
        guard !searchText.trimmingCharacters(in: .whitespaces).isEmpty else {
            return agents
        }
        let query = searchText.lowercased()
        return agents.filter {
            $0.name.lowercased().contains(query)
                || $0.slug.lowercased().contains(query)
                || $0.description.lowercased().contains(query)
                || $0.category.lowercased().contains(query)
        }
    }

    private var filteredCategories: [String] {
        let present = Set(filteredAgents.map(\.category))
        return agentCategories.filter { present.contains($0) }
    }

    private func filteredAgents(in category: String) -> [AgentEntry] {
        filteredAgents.filter { $0.category == category }
    }

    // MARK: - Body

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            searchField

            if agents.isEmpty {
                emptyState
            } else {
                agentList
                if !selectedAgents.isEmpty {
                    selectedChips
                }
            }
        }
    }

    // MARK: - Search Field

    private var searchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12))
                .foregroundColor(.inTextMuted)
            TextField("Filter agents…", text: $searchText)
                .textFieldStyle(.plain)
                .font(.inInput)
                .foregroundColor(.inTextPrimary)
                .focused($searchFocused)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 10)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(
                    searchFocused ? Color.inAccentFocus : Color.inBorderInput,
                    lineWidth: INBorder.standard
                )
        )
        .cornerRadius(INRadius.button)
    }

    // MARK: - Agent List

    private var agentList: some View {
        ScrollView(.vertical, showsIndicators: true) {
            VStack(alignment: .leading, spacing: 0) {
                if filteredAgents.isEmpty {
                    Text("No agents match your search.")
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                        .padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    ForEach(filteredCategories, id: \.self) { category in
                        categorySection(category: category)
                    }
                }
            }
        }
        .frame(maxHeight: 220)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }

    @ViewBuilder
    private func categorySection(category: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            // Category header
            Text(category.uppercased())
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)
                .padding(.horizontal, 10)
                .padding(.top, 10)
                .padding(.bottom, 4)

            // Agent rows
            ForEach(filteredAgents(in: category)) { agent in
                agentRow(agent: agent)

                if agent.id != filteredAgents(in: category).last?.id {
                    Divider()
                        .background(Color.inDivider)
                        .padding(.horizontal, 10)
                }
            }
        }
    }

    @ViewBuilder
    private func agentRow(agent: AgentEntry) -> some View {
        let isSelected = selectedAgents.contains(agent.slug)

        Button {
            if isSelected {
                selectedAgents.remove(agent.slug)
            } else {
                selectedAgents.insert(agent.slug)
            }
        } label: {
            HStack(alignment: .top, spacing: 10) {
                // Checkmark indicator
                ZStack {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(isSelected ? Color.inAccent : Color.clear)
                        .frame(width: 14, height: 14)
                    RoundedRectangle(cornerRadius: 3)
                        .stroke(isSelected ? Color.inAccent : Color.inBorderInput, lineWidth: 1)
                        .frame(width: 14, height: 14)
                    if isSelected {
                        Image(systemName: "checkmark")
                            .font(.system(size: 9, weight: .bold))
                            .foregroundColor(.white)
                    }
                }
                .padding(.top, 2)

                VStack(alignment: .leading, spacing: 2) {
                    Text(agent.name)
                        .font(.inBodyMedium)
                        .foregroundColor(isSelected ? .inAccentLight : .inTextPrimary)
                        .lineLimit(1)
                    Text(agent.description)
                        .font(.inCaption)
                        .foregroundColor(.inTextMuted)
                        .lineLimit(2)
                    Text(agent.slug)
                        .font(.inSubtitle)
                        .foregroundColor(.inTextFaint)
                        .lineLimit(1)
                }

                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(isSelected ? Color.inAccentBgSubtle : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(agent.name). \(agent.description)")
        .accessibilityHint(isSelected ? "Selected. Activate to deselect." : "Activate to select.")
    }

    // MARK: - Selected Chips

    private var selectedChips: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("SELECTED")
                .font(.inLabel)
                .foregroundColor(.inTextFaint)
                .tracking(0.5)

            FlowLayout(spacing: 4) {
                ForEach(Array(selectedAgents).sorted(), id: \.self) { slug in
                    TagChip(text: slug) {
                        selectedAgents.remove(slug)
                    }
                }
            }
        }
    }

    // MARK: - Empty State

    private var emptyState: some View {
        VStack(spacing: 6) {
            Image(systemName: "cube.box")
                .font(.system(size: 22))
                .foregroundColor(.inTextFaint)
            Text("No agents loaded")
                .font(.inCaption)
                .foregroundColor(.inTextMuted)
            Text("Use the refresh button to fetch agents from the registry.")
                .font(.inCaption)
                .foregroundColor(.inTextFaint)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
        .background(Color.inCodeBackground)
        .overlay(
            RoundedRectangle(cornerRadius: INRadius.button)
                .stroke(Color.inBorderInput, lineWidth: INBorder.standard)
        )
        .cornerRadius(INRadius.button)
    }
}
