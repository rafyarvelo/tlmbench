// Prints "<window id>\t<owner>\t<title>" for each on-screen app window whose owner name contains the argument,
// for `screencapture -l <id>` (just app-shot). Titles show only with the Screen Recording permission.
import CoreGraphics
import Foundation

let needle = CommandLine.arguments.count > 1 ? CommandLine.arguments[1].lowercased() : ""
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    as? [[String: Any]] ?? []
for w in windows where (w[kCGWindowLayer as String] as? Int) == 0 {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    if needle.isEmpty || owner.lowercased().contains(needle) {
        print("\(w[kCGWindowNumber as String] ?? 0)\t\(owner)\t\(w[kCGWindowName as String] as? String ?? "")")
    }
}
