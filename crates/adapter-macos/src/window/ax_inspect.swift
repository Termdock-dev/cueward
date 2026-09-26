guard CommandLine.arguments.count == 11,
      let limit = Int(CommandLine.arguments[8]),
      let maxDepth = Int(CommandLine.arguments[9]) else { fail("invalid inspection arguments") }

let window = bindWindow()
let rootRef = CommandLine.arguments[10]
let parts = rootRef.split(separator: ".", omittingEmptySubsequences: false)
let path = parts.compactMap { Int($0) }
guard path.first == 0, path.count == parts.count, path.count <= 13,
      path.allSatisfy({ $0 >= 0 }) else { fail("invalid inspection root") }
var root = window
var rootParentFingerprint = ""
for index in path.dropFirst() {
    let children = elements(root, kAXChildrenAttribute)
    guard index < children.count else { fail("inspection root disappeared; inspect the window again") }
    rootParentFingerprint = fingerprint(describeElement(root), parent: rootParentFingerprint)
    root = children[index]
}
var nodes: [[String: Any]] = []
var truncated = false

func walk(_ element: AXUIElement, ref: String, parent: String?, parentFingerprint: String, depth: Int) {
    if nodes.count >= limit {
        truncated = true
        return
    }

    var node = describeElement(element)
    let identity = fingerprint(node, parent: parentFingerprint)
    let children = elements(element, kAXChildrenAttribute)
    node["child_count"] = children.count
    node["fingerprint"] = identity
    node["ref"] = ref
    if let parent { node["parent_ref"] = parent }
    if let name = node["name"] as? String { node["name"] = String(name.prefix(512)) }
    if let value = node["value"] as? String { node["value"] = String(value.prefix(512)) }
    nodes.append(node)

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

let parentRef = path.count > 1 ? parts.dropLast().joined(separator: ".") : nil
walk(root, ref: rootRef, parent: parentRef, parentFingerprint: rootParentFingerprint, depth: 0)
validateCatalogWindow()
emit(["window_id": windowID, "owner_pid": pid, "root_ref": rootRef, "nodes": nodes, "truncated": truncated])
