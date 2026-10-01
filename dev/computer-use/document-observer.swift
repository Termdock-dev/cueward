import Foundation

func startDocumentObserver(_ observe: @escaping @Sendable (Timer) -> Void) -> Timer {
    Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true, block: observe)
}
