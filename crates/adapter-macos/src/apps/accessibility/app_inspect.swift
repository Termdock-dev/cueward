guard let limit = request["limit"] as? Int, (1...500).contains(limit),
      let maxDepth = request["depth"] as? Int, (0...12).contains(maxDepth) else { fail("invalid app inspection bounds") }
let roots = AppRoots()
let rootRef = request["root"] as? String
var nodes: [[String: Any]] = []
var truncated = false

func walk(_ element: AXUIElement, ref: String, parent: String?, identity: String, rootID: String, depth: Int) {
    guard nodes.count < limit else { truncated = true; return }
    var node = appNode(element)
    let digest = fingerprint(node, parent: identity)
    let children = elements(element, kAXChildrenAttribute)
    node["fingerprint"] = digest
    node["root_fingerprint"] = rootID
    node["ref"] = ref
    node["child_count"] = children.count
    if let parent { node["parent_ref"] = parent }
    if let name = node["name"] as? String { node["name"] = String(name.prefix(512)) }
    if let value = node["value"] as? String { node["value"] = String(value.prefix(512)) }
    nodes.append(node)
    if rootRef == nil { return }
    if depth >= maxDepth { if !children.isEmpty { truncated = true }; return }
    for (index, child) in children.enumerated() {
        walk(child, ref: "\(ref).\(index)", parent: ref, identity: digest, rootID: rootID, depth: depth + 1)
        if nodes.count >= limit {
            if index + 1 < children.count { truncated = true }
            break
        }
    }
}

if let rootRef {
    let (root, parentID, rootID) = resolve(rootRef, roots)
    let parts = rootRef.split(separator: ".")
    let parent = parts.count > 1 ? parts.dropLast().joined(separator: ".") : nil
    walk(root, ref: rootRef, parent: parent, identity: parentID, rootID: rootID, depth: 0)
} else {
    for (index, root) in roots.windows.enumerated() {
        walk(root, ref: "w\(index)", parent: nil, identity: roots.context, rootID: rootFingerprint(root), depth: 0)
    }
    if let menu = roots.menu {
        walk(menu, ref: "menu", parent: nil, identity: roots.context, rootID: rootFingerprint(menu), depth: 0)
    }
}
validateObservation(roots.context)
emit(["app": instance, "root_ref": rootRef as Any? ?? NSNull(), "context": roots.context,
      "nodes": nodes, "truncated": truncated])
