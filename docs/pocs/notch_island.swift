import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!

    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // 抓取主螢幕尺寸
        guard let screen = NSScreen.main else {
            NSApp.terminate(nil)
            return
        }
        
        let islandWidth: CGFloat = 320
        let islandHeight: CGFloat = 44
        
        // 定位在螢幕頂部中央（模擬瀏海下方的動態島）
        let x = screen.frame.midX - islandWidth / 2
        // y 座標為螢幕最頂端往下推一點
        let y = screen.frame.maxY - islandHeight - 12
        
        let endRect = NSRect(x: x, y: y, width: islandWidth, height: islandHeight)
        
        // 建立無邊框、透明背景的視窗
        window = NSWindow(contentRect: endRect, styleMask: .borderless, backing: .buffered, defer: false)
        window.level = .popUpMenu // 顯示在最上層，蓋過 Menu Bar
        window.backgroundColor = .clear
        window.isOpaque = false
        window.hasShadow = true
        window.ignoresMouseEvents = true // 讓滑鼠點擊可以穿透
        
        // 建立圓角黑色背景的容器
        let container = NSView(frame: NSRect(x: 0, y: 0, width: islandWidth, height: islandHeight))
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.95).cgColor
        container.layer?.cornerRadius = islandHeight / 2
        // 加上一點外框模擬質感
        container.layer?.borderWidth = 1.0
        container.layer?.borderColor = NSColor(white: 0.2, alpha: 1.0).cgColor
        
        // 加入文字標籤
        let label = NSTextField(labelWithString: "✨ 正在更新備忘錄...")
        label.textColor = .white
        label.font = NSFont.systemFont(ofSize: 15, weight: .medium)
        label.alignment = .center
        label.frame = NSRect(x: 0, y: (islandHeight - 20) / 2 - 1, width: islandWidth, height: 20)
        container.addSubview(label)
        
        window.contentView = container
        
        // 動畫起始位置（藏在螢幕上方）
        var startRect = endRect
        startRect.origin.y += islandHeight + 20
        window.setFrame(startRect, display: true)
        window.makeKeyAndOrderFront(nil)
        
        // 下拉動畫 (彈出)
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.5
            context.timingFunction = CAMediaTimingFunction(controlPoints: 0.2, 0.8, 0.2, 1.0) // 彈性阻尼感
            self.window.animator().setFrame(endRect, display: true)
        })
        
        // 3 秒後收起動畫並結束程式
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.4
                context.timingFunction = CAMediaTimingFunction(name: .easeIn)
                self.window.animator().setFrame(startRect, display: true)
                self.window.animator().alphaValue = 0.0
            }) {
                NSApp.terminate(nil)
            }
        }
    }
}

let app = NSApplication.shared
// 設定為 accessory 就不會在 Dock 顯示圖示
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
