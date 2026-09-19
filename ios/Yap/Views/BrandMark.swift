import SwiftUI

/// The speech bubble with sound bars, drawn to match the app icon.
struct BrandMark: View {
    var size: CGFloat = 30
    var live = false
    @State private var bounce = false

    private let bars: [(x: CGFloat, y: CGFloat, h: CGFloat)] = [(15, 25, 9), (23.5, 18.5, 22), (32, 21.5, 16), (40.5, 16.5, 26)]

    var body: some View {
        ZStack(alignment: .topLeading) {
            Bubble().fill(Theme.accent)
            ForEach(bars.indices, id: \.self) { i in
                let bar = bars[i]
                Capsule()
                    .fill(.white)
                    .frame(width: 5 * size / 64, height: bar.h * size / 64)
                    .scaleEffect(y: live ? (bounce ? 1.1 : 0.55) : 1)
                    .animation(live ? .easeInOut(duration: 0.45).repeatForever().delay(Double(i) * 0.12) : .default, value: bounce)
                    .offset(x: bar.x * size / 64, y: bar.y * size / 64)
            }
        }
        .frame(width: size, height: size)
        .onAppear { bounce = true }
        .accessibilityHidden(true)
    }

    private struct Bubble: Shape {
        func path(in rect: CGRect) -> Path {
            let s = rect.width / 64
            var p = Path()
            p.addRoundedRect(in: CGRect(x: 4 * s, y: 8 * s, width: 56 * s, height: 42 * s), cornerSize: CGSize(width: 12 * s, height: 12 * s))
            p.move(to: CGPoint(x: 27 * s, y: 49 * s))
            p.addLine(to: CGPoint(x: 16 * s, y: 58 * s))
            p.addLine(to: CGPoint(x: 17 * s, y: 49 * s))
            p.closeSubpath()
            return p
        }
    }
}
