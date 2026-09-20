import SwiftUI

struct KeyboardSetupView: View {
    private let steps = [
        ("gearshape", "Open Settings", "Tap the button below. It opens Moonshot's page in the Settings app."),
        ("keyboard", "Keyboards → turn on Moonshot", "Then turn on Allow Full Access. Moonshot needs it to hand your words from the app to the keyboard."),
        ("globe", "Switch to Moonshot while typing", "In any app, tap and hold 🌐 on your keyboard and pick Moonshot."),
        ("mic.fill", "Tap the mic and talk", "The first time, your phone jumps to Moonshot to turn the mic on. Tap ◀ in the top-left to go back, keep talking, then tap ■. Your words get typed."),
        ("clock", "After that, no jumping", "The mic stays ready for 5 minutes, so you can keep dictating without leaving your app."),
    ]

    var body: some View {
        List {
            ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                HStack(alignment: .top, spacing: 14) {
                    ZStack {
                        Circle().fill(Theme.accentSoft).frame(width: 36, height: 36)
                        Image(systemName: step.0).foregroundStyle(Theme.accent)
                    }
                    VStack(alignment: .leading, spacing: 4) {
                        Text("\(index + 1). \(step.1)").font(.system(.body, design: .rounded).weight(.semibold))
                        Text(step.2).font(.subheadline).foregroundStyle(Theme.ink2)
                    }
                }
                .padding(.vertical, 6)
            }
            Section {
                Button {
                    if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                } label: {
                    Label("Open Settings", systemImage: "arrow.up.forward.app").frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .listRowBackground(Color.clear)
            }
        }
        .navigationTitle("Moonshot keyboard")
    }
}
