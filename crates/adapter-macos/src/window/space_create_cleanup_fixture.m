#import <Cocoa/Cocoa.h>
#import <objc/message.h>
#import <dlfcn.h>
#import <libproc.h>
@protocol TestSpaceRemoval <NSObject>
- (instancetype)initWithSpaceID:(uint64_t)spaceID;
- (void)performWithWMBridgeDelegate;
@end
static BOOL containsSpace(NSArray *rows, NSNumber *owned) {
    for (NSDictionary *display in rows) for (NSDictionary *space in display[@"Spaces"])
        if ([space[@"ManagedSpaceID"] isEqual:owned]) return YES;
    return NO;
}
static BOOL systemDesktopWindow(NSDictionary *window) {
    char path[PROC_PIDPATHINFO_MAXSIZE] = {0};
    if (proc_pidpath([window[(id)kCGWindowOwnerPID] intValue], path, sizeof(path)) <= 0) return NO;
    NSString *executable = @(path);
    NSString *server = @"/System/Library/PrivateFrameworks/SkyLight.framework/Resources/WindowServer";
    if ([executable isEqual:server.stringByResolvingSymlinksInPath]) return YES;
    return [executable isEqual:@"/System/Library/CoreServices/WindowManager.app/Contents/MacOS/WindowManager"] &&
        [window[(id)kCGWindowLayer] intValue] < CGWindowLevelForKey(kCGDesktopWindowLevelKey);
}
static int removeDesktop(NSNumber *owned, int connection, CFArrayRef (*catalog)(int)) {
    Class cls = NSClassFromString(@"SLSBridgedSpaceDestroyOperation");
    SEL init = NSSelectorFromString(@"initWithSpaceID:"), perform = NSSelectorFromString(@"performWithWMBridgeDelegate");
    if (![cls instancesRespondToSelector:init] || ![cls instancesRespondToSelector:perform]) return 9;
    id<TestSpaceRemoval> operation = [(id<TestSpaceRemoval>)[cls alloc] initWithSpaceID:owned.unsignedLongLongValue];
    if (!operation) return 10;
    [operation performWithWMBridgeDelegate];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    do {
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        NSArray *after = CFBridgingRelease(catalog(connection));
        if (after.count && !containsSpace(after, owned)) return 0;
    } while (deadline.timeIntervalSinceNow > 0);
    fprintf(stderr, "owned test desktop removal was not confirmed\n");
    return 11;
}
// Test-only cleanup. Retain visible desktops and desktops with exclusive application windows.
int main(void) { @autoreleasepool {
    NSDictionary *request = [NSJSONSerialization JSONObjectWithData:NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile options:0 error:nil];
    NSNumber *owned = request[@"space_id"];
    if (!owned || owned.unsignedLongLongValue == 0) return 1;
    void *lib = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY | RTLD_GLOBAL);
    int (*connect)(void) = dlsym(lib, "SLSMainConnectionID");
    CFArrayRef (*catalog)(int) = dlsym(lib, "SLSCopyManagedDisplaySpaces");
    CFArrayRef (*membership)(int,int,CFArrayRef) = dlsym(lib, "SLSCopySpacesForWindows");
    if (!connect || !catalog || !membership) return 2;
    int connection = connect();
    NSArray *rows = CFBridgingRelease(catalog(connection));
    if (!rows.count) return 3;
    BOOL found = NO;
    NSMutableSet *otherSpaces = [NSMutableSet set];
    for (NSDictionary *display in rows) {
        if (!display[@"Current Space"][@"ManagedSpaceID"] ||
            [display[@"Current Space"][@"ManagedSpaceID"] isEqual:owned]) return 4;
        for (NSDictionary *space in display[@"Spaces"]) {
            if ([space[@"ManagedSpaceID"] isEqual:owned]) {
                if (![space[@"type"] isEqual:@0]) return 5;
                found = YES;
            } else if (space[@"ManagedSpaceID"]) [otherSpaces addObject:space[@"ManagedSpaceID"]];
        }
    }
    if (!found) return 0;
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll,kCGNullWindowID));
    if (!windows) return 6;
    for (NSDictionary *window in windows) {
        NSNumber *wid = window[(id)kCGWindowNumber];
        if (!wid) return 7;
        NSArray *spaces = CFBridgingRelease(membership(connection,7,(__bridge CFArrayRef)@[wid]));
        if (![spaces containsObject:owned] || systemDesktopWindow(window)) continue;
        if (![otherSpaces intersectsSet:[NSSet setWithArray:spaces]]) {
            fprintf(stderr, "owned test desktop still has an exclusive application window\n");
            return 8;
        }
    }
    return removeDesktop(owned, connection, catalog);
} }
