import SwiftUI

@main
struct YapApp: App {
    @StateObject private var engine = Engine()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(engine)
                .tint(Theme.accent)
                .task { await engine.runLaunchTest() }
                .onOpenURL { url in
                    // yap://dictate comes from the keyboard's mic button.
                    if url.host == "dictate" { engine.openedFromKeyboard() }
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
            SettingsView()
                .tabItem { Label("Settings", systemImage: "gearshape") }
                .tag("settings")
        }
    }
}
