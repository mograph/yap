import SwiftUI
import UIKit

extension UIColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha)
    }
}

extension Color {
    init(hex: UInt32) { self.init(uiColor: UIColor(hex: hex)) }

    static func adaptive(light: UInt32, dark: UInt32) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }
}

/// Same warm palette as the Mac app.
enum Theme {
    static let accent = Color.adaptive(light: 0xF0542C, dark: 0xFF6A3D)
    static let accentSoft = Color.adaptive(light: 0xFDE8DF, dark: 0x3A2219)
    static let paper = Color.adaptive(light: 0xF6F4EF, dark: 0x151412)
    static let card = Color.adaptive(light: 0xFFFEFB, dark: 0x201E1B)
    static let card2 = Color.adaptive(light: 0xF8F6F1, dark: 0x282622)
    static let ink = Color.adaptive(light: 0x1A1712, dark: 0xF4F1EA)
    static let ink2 = Color.adaptive(light: 0x5F594F, dark: 0xBDB6AA)
    static let ink3 = Color.adaptive(light: 0x928B7F, dark: 0x8A8378)
    static let line = Color.adaptive(light: 0xE9E6DF, dark: 0x2E2C29)
    static let good = Color.adaptive(light: 0x1D8A4B, dark: 0x4CC27A)
    static let goodSoft = Color.adaptive(light: 0xE0F2E6, dark: 0x1C2E22)
    static let bad = Color.adaptive(light: 0xC33F2B, dark: 0xFF7A66)
    static let badSoft = Color.adaptive(light: 0xFBE4DF, dark: 0x3A201B)
    static let warn = Color.adaptive(light: 0x8F5B00, dark: 0xF5B93A)
    static let warnSoft = Color.adaptive(light: 0xFDF0CF, dark: 0x352C17)
}
