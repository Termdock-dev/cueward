import Cocoa
import Darwin

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
func emit(_ value: Any) {
    guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else {
        fail("cannot encode application result")
    }
    FileHandle.standardOutput.write(data)
}
func canonical(_ url: URL) -> URL { url.standardizedFileURL.resolvingSymlinksInPath() }
func describe(_ app: NSRunningApplication) -> [String: Any] {
    ["pid": app.processIdentifier, "name": app.localizedName ?? "",
     "bundle_id": app.bundleIdentifier as Any? ?? NSNull(),
     "path": app.bundleURL.map { canonical($0).path } as Any? ?? NSNull(),
     "is_active": app.isActive, "is_hidden": app.isHidden,
     "finished_launching": app.isFinishedLaunching]
}
func emitLaunch(_ app: NSRunningApplication, _ status: String, _ before: pid_t) {
    guard !app.isTerminated else { fail("application exited during launch") }
    let after = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
    emit(["status": status, "app": describe(app), "frontmost_pid_before": before,
          "frontmost_pid_after": after, "foreground_changed": before != after])
}
func resolve(_ request: [String: Any]) -> URL {
    let url: URL
    if let path = request["path"] as? String { url = URL(fileURLWithPath: path) }
    else if let id = request["bundle_id"] as? String,
            let found = NSWorkspace.shared.urlForApplication(withBundleIdentifier: id) { url = found }
    else { fail("application was not found") }
    let result = canonical(url)
    guard let bundle = Bundle(url: result), bundle.bundleURL.pathExtension == "app",
          bundle.object(forInfoDictionaryKey: "CFBundlePackageType") as? String == "APPL",
          let executable = bundle.executableURL,
          FileManager.default.isExecutableFile(atPath: executable.path) else { fail("path is not an executable application bundle") }
    if let expected = request["bundle_id"] as? String, bundle.bundleIdentifier != expected {
        fail("resolved application bundle id does not match")
    }
    return result
}
func existing(_ url: URL) -> NSRunningApplication? {
    let apps = NSWorkspace.shared.runningApplications.filter { !$0.isTerminated }
    let exact = apps.filter { $0.bundleURL.map { canonical($0) == url } ?? false }
    guard exact.count <= 1 else { fail("multiple application instances are running; select a window by PID") }
    if let app = exact.first { return app }
    if let id = Bundle(url: url)?.bundleIdentifier,
       apps.contains(where: { $0.bundleIdentifier == id }) {
        fail("this bundle id is already running from another path; select that app explicitly")
    }
    return nil
}
func launch(_ request: [String: Any]) {
    let url = resolve(request)
    let before = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
    if let app = existing(url) { emitLaunch(app, "already_running", before); return }
    guard let caller = request["caller_pid"] as? Int32, caller > 0, kill(caller, 0) == 0 else {
        fail("launch caller exited")
    }
    let config = NSWorkspace.OpenConfiguration()
    config.activates = false
    config.addsToRecentItems = false
    config.hidesOthers = false
    config.promptsUserIfNeeded = false
    config.createsNewApplicationInstance = false
    config.allowsRunningApplicationSubstitution = false
    var finished = false
    NSWorkspace.shared.openApplication(at: url, configuration: config) { app, error in
        guard let app = app, error == nil else { fail("application launch failed; inspect running apps before retrying") }
        guard app.bundleURL.map({ canonical($0) == url }) == true else { fail("launched application path differs from request") }
        emitLaunch(app, "launched", before)
        finished = true
    }
    let deadline = Date().addingTimeInterval(20)
    while !finished && deadline.timeIntervalSinceNow > 0 {
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    }
    if !finished { fail("launch completion timed out; application may still start") }
}
alarm(30)
guard let request = try? JSONSerialization.jsonObject(with: FileHandle.standardInput.readDataToEndOfFile()) as? [String: Any] else {
    fail("invalid application request")
}
switch request["action"] as? String {
case "list":
    emit(NSWorkspace.shared.runningApplications.filter {
        !$0.isTerminated && $0.activationPolicy != .prohibited
    }.sorted { $0.processIdentifier < $1.processIdentifier }.map(describe))
case "launch": launch(request)
default: fail("unsupported application action")
}
