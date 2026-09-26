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
func executableBundle(_ url: URL) -> Bundle? {
    guard let bundle = Bundle(url: url), bundle.bundleURL.pathExtension == "app",
          bundle.object(forInfoDictionaryKey: "CFBundlePackageType") as? String == "APPL",
          let executable = bundle.executableURL,
          FileManager.default.isExecutableFile(atPath: executable.path) else { return nil }
    return bundle
}
func resolveBundleID(_ id: String) -> URL {
    guard #available(macOS 12.0, *) else { fail("bundle discovery requires macOS 12 or later; use --path") }
    let matches = Set(NSWorkspace.shared.urlsForApplications(withBundleIdentifier: id).map(canonical).filter {
        executableBundle($0)?.bundleIdentifier == id
    })
    guard matches.count <= 1 else { fail("multiple application installations match this bundle id; use --path") }
    guard let found = matches.first else { fail("application was not found") }
    return found
}
func resolve(_ request: [String: Any]) -> URL {
    let url: URL
    if let path = request["path"] as? String { url = URL(fileURLWithPath: path) }
    else if let id = request["bundle_id"] as? String { url = resolveBundleID(id) }
    else { fail("application was not found") }
    let result = canonical(url)
    guard let bundle = executableBundle(result) else { fail("path is not an executable application bundle") }
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
    let completion = LaunchCompletion<(NSRunningApplication?, Bool)>(deadline: ProcessInfo.processInfo.systemUptime + 20)
    NSWorkspace.shared.openApplication(at: url, configuration: config) { app, error in
        completion.complete((app, error == nil))
    }
    while true {
        switch completion.poll() {
        case .pending: RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        case .timedOut: fail("launch completion timed out; application may still start")
        case .completed(let result):
            guard let app = result.0, result.1 else { fail("application launch failed; inspect running apps before retrying") }
            guard app.bundleURL.map({ canonical($0) == url }) == true else { fail("launched application path differs from request") }
            emitLaunch(app, "launched", before)
            return
        }
    }
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
