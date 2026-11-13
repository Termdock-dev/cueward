import CoreGraphics
import Foundation

// Read only the requested IDs; never print other windows or their titles/contents.
let arguments = Array(CommandLine.arguments.dropFirst())
let ids = arguments.compactMap(UInt32.init)
guard (1...4).contains(ids.count), ids.count == arguments.count,
      ids.allSatisfy({ $0 > 0 }), Set(ids).count == ids.count else {
    fputs("usage: swift window-catalog-contrast.swift <owned-window-id> [up to four unique IDs]\n", stderr)
    exit(2)
}
guard let all = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID)
        as? [[String: Any]] else {
    fputs("complete window catalog unavailable\n", stderr)
    exit(1)
}
func matching(_ rows: [[String: Any]], id: UInt32) -> [[String: Any]] {
    rows.filter { ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == id }
}
func identity(_ row: [String: Any]) -> NSDictionary {
    let keys = [kCGWindowNumber, kCGWindowOwnerPID, kCGWindowName, kCGWindowBounds]
    return Dictionary(uniqueKeysWithValues: keys.map { ($0 as String, row[$0 as String] ?? NSNull()) }) as NSDictionary
}
let results: [[String: Any]] = ids.map { id in
    let full = matching(all, id: id)
    let raw = CGWindowListCopyWindowInfo(.optionIncludingWindow, id)
    let single = matching(raw as? [[String: Any]] ?? [], id: id)
    let inputRaw = CGWindowListCopyWindowInfo([.optionIncludingWindow, .excludeDesktopElements], id)
    let input = matching(inputRaw as? [[String: Any]] ?? [], id: id)
    return ["window_id": id, "all_matches": full.count,
            "including_available": raw != nil, "including_matches": single.count,
            "including_excluding_desktop_available": inputRaw != nil,
            "including_excluding_desktop_matches": input.count,
            "identity_equal_when_unique": full.count == 1 && single.count == 1
                ? identity(full[0]).isEqual(identity(single[0])) as Any : NSNull()]
}
let output: [String: Any] = ["schema": 1, "kind": "read_only_catalog_contrast",
    "macos": ProcessInfo.processInfo.operatingSystemVersionString, "results": results]
let data = try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
