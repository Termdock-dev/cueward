import Cocoa

func workspaceConfiguration(newInstance: Bool = false) -> NSWorkspace.OpenConfiguration {
    let config = NSWorkspace.OpenConfiguration()
    config.activates = false
    config.addsToRecentItems = false
    config.hidesOthers = false
    config.promptsUserIfNeeded = false
    config.createsNewApplicationInstance = newInstance
    config.allowsRunningApplicationSubstitution = false
    return config
}

func awaitApplication(_ completion: LaunchCompletion<(NSRunningApplication?, Bool)>,
                      at url: URL, operation: String) -> NSRunningApplication {
    while true {
        switch completion.poll() {
        case .pending: RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        case .timedOut: fail("\(operation) completion timed out; request may still complete")
        case .completed(let result):
            guard let app = result.0, result.1 else {
                fail("application \(operation) failed; inspect running apps before retrying")
            }
            guard app.bundleURL.map({ canonical($0) == url }) == true else {
                fail("returned application path differs from request; delivery is uncertain")
            }
            return app
        }
    }
}
