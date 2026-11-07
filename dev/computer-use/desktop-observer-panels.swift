import Darwin

/// AppKit starts one of these per client when a panel opens. The observer watches this name only.
let panelServiceExecutable = "com.apple.appkit.xpc.openAndSavePanelService"

/// Read-only kernel process listing. It never signals, attaches to or messages a process.
struct KernelProcessListing {
    private var entries = [kinfo_proc](repeating: kinfo_proc(), count: 2048)

    private mutating func snapshot() throws -> Int {
        var request: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_ALL, 0]
        let stride = MemoryLayout<kinfo_proc>.stride
        while true {
            var bytes = entries.count * stride
            if sysctl(&request, 4, &entries, &bytes, nil, 0) == 0 { return bytes / stride }
            guard errno == ENOMEM, entries.count < 65_536 else {
                throw ObservationError.invalid("kernel process listing failed")
            }
            entries = [kinfo_proc](repeating: kinfo_proc(), count: entries.count * 2)
        }
    }

    private func executableName(_ pid: Int32) -> String? {
        var path = [UInt8](repeating: 0, count: 4 * Int(MAXPATHLEN))
        let length = proc_pidpath(pid, &path, UInt32(path.count))
        guard length > 0 else { return nil }
        return String(decoding: path.prefix(Int(length)), as: UTF8.self).split(separator: "/").last.map(String.init)
    }

    private func startTicks(_ pid: Int32) -> UInt64? {
        var usage = rusage_info_v4()
        let status = withUnsafeMutablePointer(to: &usage) { pointer in
            pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(pid, RUSAGE_INFO_V4, $0) }
        }
        return status == 0 && usage.ri_proc_start_abstime > 0 ? usage.ri_proc_start_abstime : nil
    }

    /// List processes with this exact executable name. Listing every time, rather than only new
    /// PIDs, matters because launchd starts a service as `xpcproxy`, which then becomes the service
    /// under the same PID. A process that exits during the read is left out.
    mutating func processes(named name: String) throws -> [ListedProcess] {
        let count = try snapshot()
        // The kernel keeps only the start of the name, so the full path settles each candidate.
        let prefix = Array(name.utf8.prefix(Int(MAXCOMLEN)))
        var result: [ListedProcess] = []
        for entry in entries.prefix(count) {
            let candidate = withUnsafeBytes(of: entry.kp_proc.p_comm) { memcmp($0.baseAddress, prefix, prefix.count) == 0 }
            let pid = entry.kp_proc.p_pid
            if candidate, executableName(pid) == name, let ticks = startTicks(pid) {
                result.append(ListedProcess(pid: pid, startTicks: ticks))
            }
        }
        return result
    }
}
