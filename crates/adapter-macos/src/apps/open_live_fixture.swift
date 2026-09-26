import Cocoa
import Darwin

final class Delegate: NSObject, NSApplicationDelegate {
    var activations = 0
    var documents: [[String:String]] = []
    var window: NSWindow?
    let stateURL = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("state-\(getpid()).json")
    func save() {
        let value: [String:Any] = ["pid":getpid(),"activations":activations,"documents":documents]
        try? JSONSerialization.data(withJSONObject:value).write(to:stateURL,options:.atomic)
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(contentRect:NSRect(x:90,y:90,width:400,height:240),
            styleMask:[.titled,.closable],backing:.buffered,defer:false)
        window.title = "Cueward Document Fixture"
        window.orderBack(nil)
        self.window = window
        save()
    }
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        for filename in filenames {
            do { documents.append(["file":filename,"content":try String(contentsOfFile:filename,encoding:.utf8)]) }
            catch { documents.append(["file":filename,"error":String(describing:error)]) }
        }
        save()
        sender.reply(toOpenOrPrint:.success)
    }
    func applicationDidBecomeActive(_ notification: Notification) { activations += 1; save() }
}
alarm(90)
let app = NSApplication.shared
let delegate = Delegate()
app.setActivationPolicy(.regular)
app.delegate = delegate
app.run()
