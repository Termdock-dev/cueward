import Cocoa
import ObjectiveC
let fixtureURLs = CommandLine.arguments.dropFirst().map { URL(fileURLWithPath: $0) as NSURL }
let single: @convention(block) (AnyObject, NSString) -> NSURL? = { _, _ in fixtureURLs.first }
let plural: @convention(block) (AnyObject, NSString) -> NSArray = { _, _ in fixtureURLs as NSArray }
let cls = object_getClass(NSWorkspace.shared)!
let singleSelector = #selector(NSWorkspace.urlForApplication(withBundleIdentifier:))
let pluralSelector = #selector(NSWorkspace.urlsForApplications(withBundleIdentifier:))
class_replaceMethod(cls, singleSelector, imp_implementationWithBlock(single), method_getTypeEncoding(class_getInstanceMethod(cls, singleSelector)!))
class_replaceMethod(cls, pluralSelector, imp_implementationWithBlock(plural), method_getTypeEncoding(class_getInstanceMethod(cls, pluralSelector)!))
