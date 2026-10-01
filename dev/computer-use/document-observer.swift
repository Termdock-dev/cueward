import AppKit

func startDocumentObserver(_ observe: @escaping @Sendable (Timer) -> Void) -> Timer {
    let timer = Timer(timeInterval: 0.01, repeats: true, block: observe)
    // Register explicitly instead of depending on an application's common-mode set.
    for mode in [RunLoop.Mode.default, .modalPanel, .eventTracking] {
        RunLoop.main.add(timer, forMode: mode)
    }
    return timer
}
