#import <Cocoa/Cocoa.h>

// Replace only the WindowServer read. The real helper entry is compiled but never run.
static NSArray *individualCatalog;
static NSArray *fullCatalog;
static BOOL catalogUnavailable;
static CFArrayRef fixtureWindowInfo(CGWindowListOption options, CGWindowID relative) CF_RETURNS_RETAINED;
static CFArrayRef fixtureWindowInfo(CGWindowListOption options, CGWindowID relative) {
    if (options & kCGWindowListOptionIncludingWindow)
        return CFBridgingRetain(individualCatalog ?: fullCatalog);
    if (relative != kCGNullWindowID || catalogUnavailable) return NULL;
    return CFBridgingRetain(fullCatalog);
}
#define CGWindowListCopyWindowInfo fixtureWindowInfo
#define main spaceHelperEntry
