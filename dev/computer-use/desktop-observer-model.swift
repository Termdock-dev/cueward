import Foundation
import CoreFoundation

enum ObservationError: Error, CustomStringConvertible {
    case invalid(String)
    var description: String {
        switch self { case .invalid(let message): return message }
    }
}

struct ObserverConfiguration {
    let pids: [Int32]
    let intervalMS: Double
    let durationMS: Double

    init(_ value: [String: Any]) throws {
        guard let raw = value["pids"] as? [Any], !raw.isEmpty, raw.count <= 32 else {
            throw ObservationError.invalid("pids must contain 1..32 explicitly owned receiver processes")
        }
        pids = try raw.map { item in
            guard let number = wholeNumber(item), number > 0, number <= Int64(Int32.max) else {
                throw ObservationError.invalid("invalid receiver PID")
            }
            return Int32(number)
        }
        guard Set(pids).count == pids.count else { throw ObservationError.invalid("duplicate receiver PID") }
        guard let interval = finiteNumber(value["interval_ms"]), (5...50).contains(interval),
              let duration = finiteNumber(value["duration_ms"]), (100...1_800_000).contains(duration) else {
            throw ObservationError.invalid("interval_ms must be 5..50; duration_ms must be 100..1800000")
        }
        intervalMS = interval
        durationMS = duration
    }
}

func finiteNumber(_ value: Any?) -> Double? {
    guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
          number.doubleValue.isFinite else { return nil }
    return number.doubleValue
}

func wholeNumber(_ value: Any?) -> Int64? {
    guard let number = finiteNumber(value), number.rounded() == number,
          number >= 0, number < Double(Int64.max) else { return nil }
    return Int64(number)
}

/// Correlate every online display with a current Space; never silently omit a display.
func correlateDisplaySpaces(_ rows: [[String: Any]], online: [String: String],
                            mainUUID: String) throws -> [String: String] {
    guard !online.isEmpty, Set(online.values).count == online.count else {
        throw ObservationError.invalid("online display identities are missing or ambiguous")
    }
    var catalog: [String: String] = [:]
    for row in rows {
        guard let identifier = row["Display Identifier"] as? String, !identifier.isEmpty,
              let current = row["Current Space"] as? [String: Any],
              let space = wholeNumber(current["ManagedSpaceID"]), space > 0 else {
            throw ObservationError.invalid("managed display has no current Space identity")
        }
        let key = identifier == "Main" ? mainUUID.uppercased() : identifier.uppercased()
        guard catalog[key] == nil else { throw ObservationError.invalid("duplicate managed display identity") }
        catalog[key] = String(space)
    }
    var result: [String: String] = [:]
    for (display, uuid) in online {
        guard let space = catalog[uuid.uppercased()] else {
            throw ObservationError.invalid("online display \(display) has no independently observed visible Space")
        }
        result[display] = space
    }
    return result
}

/// Parse bounded newline JSON control messages; unknown commands never mutate the desktop.
func observerCommand(_ data: Data) throws -> (String, String?) {
    guard data.count <= 4096,
          let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let command = value["command"] as? String else {
        throw ObservationError.invalid("invalid observer control message")
    }
    if command == "stop" { return (command, nil) }
    guard command == "mark", let id = value["id"] as? String,
          !id.isEmpty, id.utf8.count <= 128 else {
        throw ObservationError.invalid("expected stop or mark with a bounded ID")
    }
    return (command, id)
}
