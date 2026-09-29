import SwiftUI

@main
struct YapApp: App {
    @StateObject private var engine = Engine()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(engine)
                .tint(Theme.accent)
                .task { await engine.runLaunchTest() }
                .onOpenURL { url in
                    // moonshot://dictate comes from the keyboard's mic button.
                    if url.host == "dictate" { engine.openedFromKeyboard() }
                }
                // Coming back to Moonshot picks up whatever your other devices did meanwhile.
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active { Task { await engine.syncCloud(explicit: false) } }
                }
        }
    }
}

struct RootView: View {
    /// `-tab voice` on launch opens that tab (handy for screenshots).
    @State private var tab = UserDefaults.standard.string(forKey: "tab") ?? "talk"

    var body: some View {
        TabView(selection: $tab) {
            TalkView()
                .tabItem { Label("Talk", systemImage: "waveform") }
                .tag("talk")
            VoiceView()
                .tabItem { Label("Your voice", systemImage: "slider.horizontal.3") }
                .tag("voice")
            InsightsView()
                .tabItem { Label("Insights", systemImage: "chart.bar") }
                .tag("insights")
            SettingsView()
                .tabItem { Label("Settings", systemImage: "gearshape") }
                .tag("settings")
        }
    }
}
