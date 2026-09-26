import ApplicationServices
import Foundation

enum AXElementsError: Error, CustomStringConvertible {
    case readFailed(AXError)
    case invalidValue

    var description: String {
        switch self {
        case .readFailed(let error): return "AX read failed (\(error.rawValue))"
        case .invalidValue: return "AX read returned an invalid element array"
        }
    }
}

func checkedElements(status: AXError, value: CFTypeRef?) throws -> [AXUIElement] {
    switch status {
    case .attributeUnsupported, .noValue:
        // Leaf elements may omit AXChildren entirely.
        return []
    case .success:
        guard let values = value as? [AnyObject],
              values.allSatisfy({ CFGetTypeID($0) == AXUIElementGetTypeID() }) else {
            throw AXElementsError.invalidValue
        }
        return values.map { $0 as! AXUIElement }
    default:
        throw AXElementsError.readFailed(status)
    }
}
