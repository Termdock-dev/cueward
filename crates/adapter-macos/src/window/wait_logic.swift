func selectorMatches(_ node: [String: Any], _ selector: [String: Any]) -> Bool {
    guard node["role"] as? String == selector["role"] as? String else { return false }
    for key in ["name", "identifier"] {
        if let expected = selector[key] as? String, node[key] as? String != expected { return false }
    }
    return true
}

func evaluateWait(_ condition: String, _ matches: [[String: Any]], complete: Bool, value: String?) -> String {
    switch condition {
    case "element-exists": return matches.isEmpty ? "pending" : "matched"
    case "element-absent": return complete && matches.isEmpty ? "matched" : "pending"
    case "value-equals", "enabled":
        if matches.count > 1 { return "ambiguous" }
        guard complete, let node = matches.first else { return "pending" }
        let matched = condition == "enabled" ? node["enabled"] as? Bool == true :
            value != nil && node["value"] as? String == value
        return matched ? "matched" : "pending"
    default: return "pending"
    }
}
