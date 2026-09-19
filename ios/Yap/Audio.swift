import AVFoundation

/// Microphone capture at 16 kHz mono (what Whisper wants). The engine can stay running in the
/// background during a keyboard session; `capturing` decides whether audio is kept.
final class AudioCapture {
    private let engine = AVAudioEngine()
    private let target = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false)!
    private var converter: AVAudioConverter?
    private let lock = NSLock()
    private var samples: [Float] = []
    private var capturing = false

    /// Mic level (RMS) while capturing, for the waveform.
    var onLevel: ((Float) -> Void)?
    var isRunning: Bool { engine.isRunning }

    /// Must be called while the app is in the foreground; iOS won't start the mic from the background.
    func startEngine() throws {
        guard !engine.isRunning else { return }
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playAndRecord, mode: .default, options: [.mixWithOthers, .defaultToSpeaker])
        try session.setActive(true)
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        converter = AVAudioConverter(from: format, to: target)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            self?.receive(buffer)
        }
        engine.prepare()
        try engine.start()
    }

    func stopEngine() {
        guard engine.isRunning else { return }
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    func beginCapture() {
        lock.lock()
        samples.removeAll(keepingCapacity: true)
        capturing = true
        lock.unlock()
    }

    /// Stops keeping audio and returns everything captured since `beginCapture`.
    func endCapture() -> [Float] {
        lock.lock()
        capturing = false
        let out = samples
        samples = []
        lock.unlock()
        return out
    }

    private func receive(_ buffer: AVAudioPCMBuffer) {
        lock.lock()
        let on = capturing
        lock.unlock()
        guard on, let converter else { return }

        if let channel = buffer.floatChannelData?[0], buffer.frameLength > 0 {
            var sum: Float = 0
            for i in 0..<Int(buffer.frameLength) { sum += channel[i] * channel[i] }
            let rms = (sum / Float(buffer.frameLength)).squareRoot()
            DispatchQueue.main.async { self.onLevel?(rms) }
        }

        let ratio = target.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 32
        guard let out = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: capacity) else { return }
        var fed = false
        var error: NSError?
        converter.convert(to: out, error: &error) { _, status in
            if fed {
                status.pointee = .noDataNow
                return nil
            }
            fed = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil, let data = out.floatChannelData?[0] else { return }
        let chunk = Array(UnsafeBufferPointer(start: data, count: Int(out.frameLength)))
        lock.lock()
        if capturing { samples.append(contentsOf: chunk) }
        lock.unlock()
    }
}
