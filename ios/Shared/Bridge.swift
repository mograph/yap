import Foundation

/// How the keyboard and the app talk. Keyboards can't use the microphone, so the app records
/// and the keyboard asks it to start and stop, then types whatever comes back.
///
/// Keyboard -> app: Darwin notifications ("start", "stop", "cancel").
/// App -> keyboard: shared UserDefaults, which the keyboard polls while it's on screen.
enum Bridge {
    static let defaults = UserDefaults(suiteName: Store.group) ?? .standard

    enum Key {
        /// Seconds since 1970, written every second while the app's mic session is alive.
        static let heartbeat = "heartbeat"
        /// "idle", "listening", "thinking", "done" or "error"
        static let state = "state"
        static let message = "message"
        static let result = "result"
        static let resultId = "resultId"
        /// A keyboard dictation finished and hasn't been typed yet.
        static let pending = "pending"
    }

    static var sessionAlive: Bool {
        Date().timeIntervalSince1970 - defaults.double(forKey: Key.heartbeat) < 3
    }

    static var state: String { defaults.string(forKey: Key.state) ?? "idle" }

    private static let prefix = "io.tinkerstudio.moonshot."
    private static var handlers: [String: () -> Void] = [:]

    static func post(_ name: String) {
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName((prefix + name) as CFString), nil, nil, true)
    }

    static func observe(_ name: String, _ handler: @escaping () -> Void) {
        let full = prefix + name
        handlers[full] = handler
        CFNotificationCenterAddObserver(
            CFNotificationCenterGetDarwinNotifyCenter(), nil,
            { _, _, name, _, _ in
                guard let key = name?.rawValue as String? else { return }
                DispatchQueue.main.async { Bridge.handlers[key]?() }
            },
            full as CFString, nil, .deliverImmediately)
    }
}
