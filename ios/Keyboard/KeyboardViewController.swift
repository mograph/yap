import SwiftUI
import UIKit

final class KeyboardModel: ObservableObject {
    @Published var state = "idle"
    @Published var status = "Tap to talk"
    @Published var sessionAlive = false
    @Published var needsFullAccess = false
    @Published var lastResult = ""
    @Published var showsGlobe = true
}

/// The Moonshot keyboard. It can't use the microphone itself, so it asks the Moonshot app to listen and
/// types whatever comes back.
final class KeyboardViewController: UIInputViewController {
    private let model = KeyboardModel()
    private var timer: Timer?

    override func viewDidLoad() {
        super.viewDidLoad()
        let root = KeyboardView(
            model: model,
            onMic: { [weak self] in self?.micTapped() },
            onSpace: { [weak self] in self?.textDocumentProxy.insertText(" ") },
            onDelete: { [weak self] in self?.textDocumentProxy.deleteBackward() },
            onReturn: { [weak self] in self?.textDocumentProxy.insertText("\n") },
            onGlobe: { [weak self] in self?.advanceToNextInputMode() },
            onPasteLast: { [weak self] in
                guard let self, !self.model.lastResult.isEmpty else { return }
                self.textDocumentProxy.insertText(self.model.lastResult)
            },
            onOpenApp: { [weak self] in self?.open(URL(string: "moonshot://")!) })
        let host = UIHostingController(rootView: root)
        host.view.backgroundColor = .clear
        addChild(host)
        view.addSubview(host.view)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        let height = view.heightAnchor.constraint(equalToConstant: 262)
        height.priority = .defaultHigh
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            height,
        ])
        host.didMove(toParent: self)
        model.lastResult = Bridge.defaults.string(forKey: Bridge.Key.result) ?? ""
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        model.needsFullAccess = !hasFullAccess
        model.showsGlobe = needsInputModeSwitchKey
        poll()
        timer = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { [weak self] _ in self?.poll() }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        timer?.invalidate()
        timer = nil
    }

    private func poll() {
        let defaults = Bridge.defaults
        let alive = Bridge.sessionAlive
        let state = alive ? Bridge.state : "idle"
        model.sessionAlive = alive
        model.state = state
        switch state {
        case "listening": model.status = "Listening… tap ■ when you're done"
        case "thinking": model.status = defaults.string(forKey: Bridge.Key.message) ?? "Thinking…"
        case "error": model.status = defaults.string(forKey: Bridge.Key.message) ?? "Something went wrong"
        default: model.status = alive ? "Tap to talk" : "Tap the mic to wake up Moonshot"
        }
        // A keyboard dictation finished: type it, once.
        if defaults.bool(forKey: Bridge.Key.pending), let text = defaults.string(forKey: Bridge.Key.result), !text.isEmpty {
            defaults.set(false, forKey: Bridge.Key.pending)
            textDocumentProxy.insertText(text)
            model.lastResult = text
        }
    }

    private func micTapped() {
        guard hasFullAccess else {
            model.needsFullAccess = true
            return
        }
        if Bridge.sessionAlive {
            Bridge.post(Bridge.state == "listening" ? "stop" : "start")
        } else {
            // The app has to be in front to turn the mic on. It starts listening right away;
            // come back here and tap ■ when you're done.
            model.status = "Opening Moonshot…"
            open(URL(string: "moonshot://dictate")!)
        }
    }

    /// Keyboards can't call UIApplication.open directly, so find the app through the responder chain.
    private func open(_ url: URL) {
        let selector = NSSelectorFromString("openURL:options:completionHandler:")
        var responder: UIResponder? = self
        while let current = responder {
            if let app = current as? UIApplication, app.responds(to: selector) {
                typealias OpenURL = @convention(c) (AnyObject, Selector, NSURL, NSDictionary, AnyObject?) -> Void
                let open = unsafeBitCast(app.method(for: selector), to: OpenURL.self)
                open(app, selector, url as NSURL, NSDictionary(), nil)
                return
            }
            responder = current.next
        }
    }
}
