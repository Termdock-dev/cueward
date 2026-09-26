func expectFailure(_ status: AXError, _ value: CFTypeRef?, readError: Bool) {
    do {
        _ = try checkedElements(status: status, value: value)
        fatalError("expected enumeration failure for \(status.rawValue)")
    } catch AXElementsError.readFailed(let error) {
        precondition(readError && error == status)
    } catch AXElementsError.invalidValue {
        precondition(!readError)
    } catch {
        fatalError("unexpected error: \(error)")
    }
}

do {
    let empty = try checkedElements(status: .success, value: [] as NSArray)
    let unsupported = try checkedElements(status: .attributeUnsupported, value: nil)
    let absent = try checkedElements(status: .noValue, value: nil)
    precondition(empty.isEmpty && unsupported.isEmpty && absent.isEmpty)
    let element = AXUIElementCreateApplication(1)
    let result = try checkedElements(status: .success, value: [element] as NSArray)
    precondition(result.count == 1 && CFEqual(result[0], element))
} catch {
    fatalError("valid element read failed: \(error)")
}
for status: AXError in [.cannotComplete, .invalidUIElement, .apiDisabled, .failure] {
    expectFailure(status, nil, readError: true)
}
expectFailure(.success, nil, readError: false)
expectFailure(.success, "invalid" as NSString, readError: false)
expectFailure(.success, ["invalid"] as NSArray, readError: false)
print("passed")
