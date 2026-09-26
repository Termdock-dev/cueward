// Only workspace/process boundaries are replaced. File and bundle validation,
// configuration, completion handling, recipient checks and result construction are production code.
import Cocoa
import Darwin

var scenario: String { ProcessInfo.processInfo.environment["OPEN_SCENARIO"]! }
var fixtureURL: URL { URL(fileURLWithPath: ProcessInfo.processInfo.environment["OPEN_APP"]!) }
var traceURL: URL { URL(fileURLWithPath: ProcessInfo.processInfo.environment["OPEN_TRACE"]!) }
var lockAcquired = false

final class NSRunningApplication: @unchecked Sendable {
    let processIdentifier: pid_t
    let bundleURL: URL?
    init(_ pid: pid_t, _ url: URL? = fixtureURL) { processIdentifier = pid; bundleURL = url }
    var localizedName: String? { "Fixture" }
    var bundleIdentifier: String? { "org.example.openfixture" }
    var isTerminated: Bool { scenario == "terminated-result" && processIdentifier == 201 }
    var isActive: Bool { false }
    var isHidden: Bool { false }
    var isFinishedLaunching: Bool { true }
    var activationPolicy: NSApplication.ActivationPolicy { .regular }
}

final class NSWorkspace {
    static let shared = NSWorkspace()
    final class OpenConfiguration {
        var activates = true
        var addsToRecentItems = true
        var hidesOthers = true
        var promptsUserIfNeeded = true
        var createsNewApplicationInstance = false
        var allowsRunningApplicationSubstitution = true
    }
    var runningApplications: [NSRunningApplication] {
        switch scenario {
        case "post-lock-recipient": return [NSRunningApplication(lockAcquired ? 101 : 100)]
        case "existing", "foreground", "new-instance", "new-reused", "new-foreground", "recipient-changed", "lock-busy":
            return [NSRunningApplication(100)]
        case "ambiguous": return [NSRunningApplication(100), NSRunningApplication(101)]
        case "other-path": return [NSRunningApplication(100, fixtureURL.deletingLastPathComponent().appendingPathComponent("Other.app"))]
        default: return []
        }
    }
    var frontmostApplication: NSRunningApplication? {
        if scenario == "foreground-missing" { return nil }
        if scenario == "foreground-zero" { return NSRunningApplication(0) }
        if ["foreground", "new-foreground"].contains(scenario) { return NSRunningApplication(100) }
        if scenario == "foreground-changed", FileManager.default.fileExists(atPath: traceURL.path) { return NSRunningApplication(201) }
        return NSRunningApplication(1)
    }
    func urlsForApplications(withBundleIdentifier id: String) -> [URL] { [fixtureURL] }
    func openApplication(at url: URL, configuration: OpenConfiguration,
                         completionHandler: @escaping (NSRunningApplication?, Error?) -> Void) {
        open([], withApplicationAt: url, configuration: configuration, completionHandler: completionHandler)
    }
    func open(_ files: [URL], withApplicationAt url: URL, configuration: OpenConfiguration,
              completionHandler: @escaping (NSRunningApplication?, Error?) -> Void) {
        let prior = (try? Data(contentsOf:traceURL)).flatMap { try? JSONSerialization.jsonObject(with:$0) as? [String:Any] }
        let record: [String:Any] = ["calls":(prior?["calls"] as? Int ?? 0)+1,
            "files":files.map { $0.path }, "activates":configuration.activates,
            "recent_items":configuration.addsToRecentItems,"hides_others":configuration.hidesOthers,
            "prompts":configuration.promptsUserIfNeeded,"new_instance":configuration.createsNewApplicationInstance,
            "substitution":configuration.allowsRunningApplicationSubstitution]
        try! JSONSerialization.data(withJSONObject:record).write(to:traceURL)
        if scenario == "callback-empty" { completionHandler(nil,nil); return }
        let path = scenario == "callback-other-path" ? url.appendingPathComponent("Other.app") : url
        let pid: pid_t = ["existing","new-reused"].contains(scenario) ? 100 : 201
        completionHandler(NSRunningApplication(pid,path), scenario == "callback-error" ? NSError(domain:"Fixture",code:1) : nil)
    }
}
func kill(_ pid: pid_t, _ signal: Int32) -> Int32 {
    scenario == "dead-caller" ? -1 : Darwin.kill(pid, signal)
}
func flock(_ descriptor: Int32, _ operation: Int32) -> Int32 {
    if scenario == "lock-busy" { return -1 }
    typealias NativeFlock = @convention(c) (Int32, Int32) -> Int32
    let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "flock")!
    let result = unsafeBitCast(symbol, to: NativeFlock.self)(descriptor, operation)
    if result == 0 { lockAcquired = true }
    return result
}
