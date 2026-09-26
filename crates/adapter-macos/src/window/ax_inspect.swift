guard CommandLine.arguments.count == 10,
      let limit = Int(CommandLine.arguments[8]),
      let maxDepth = Int(CommandLine.arguments[9]) else { fail("invalid inspection arguments") }

let window = bindWindow()
var nodes: [[String: Any]] = []
var truncated = false

func walk(_ element: AXUIElement, ref: String, parent: String?, parentFingerprint: String, depth: Int) {
    if nodes.count >= limit {
        truncated = true
        return
    }

    var node = describeElement(element)
    let identity = fingerprint(node, parent: parentFingerprint)
    node["fingerprint"] = identity
    node["ref"] = ref
    if let parent { node["parent_ref"] = parent }
    if let name = node["name"] as? String { node["name"] = String(name.prefix(512)) }
    if let value = node["value"] as? String { node["value"] = String(value.prefix(512)) }
    nodes.append(node)

    let children = elements(element, kAXChildrenAttribute)
    if depth >= maxDepth {
        if !children.isEmpty { truncated = true }
        return
    }
    for (index, child) in children.enumerated() {
        walk(child, ref: "\(ref).\(index)", parent: ref, parentFingerprint: identity, depth: depth + 1)
        if nodes.count >= limit {
            if index + 1 < children.count { truncated = true }
            break
        }
    }
}

walk(window, ref: "0", parent: nil, parentFingerprint: "", depth: 0)
validateCatalogWindow()
emit(["window_id": windowID, "owner_pid": pid, "nodes": nodes, "truncated": truncated])
