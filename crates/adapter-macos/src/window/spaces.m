#import <Cocoa/Cocoa.h>
#import <objc/message.h>
#import <dlfcn.h>
#import <sys/file.h>
#import <fcntl.h>
#import <signal.h>

typedef int (*Connection)(void);
typedef CFArrayRef (*DisplaySpaces)(int);
typedef CFArrayRef (*WindowSpaces)(int, int, CFArrayRef);
static DisplaySpaces displaySpaces;
static WindowSpaces windowSpaces;
static int connection;
static NSNumber *boolean(BOOL value) { return value ? @YES : @NO; }

static void fail(NSString *message) {
    fprintf(stderr, "%s\n", message.UTF8String);
    exit(1);
}
static void emit(id value) {
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:&error];
    if (!data) fail(@"cannot encode Space result");
    fwrite(data.bytes, 1, data.length, stdout);
    fputc('\n', stdout);
}
static NSArray *displays(void) {
    NSArray *rows = CFBridgingRelease(displaySpaces(connection));
    if (!rows.count) fail(@"managed display Space catalog is unavailable");
    if (!visibleSpaces(rows)) fail(@"current Space is unavailable for a display; observe before retrying");
    return rows;
}
static NSArray *membership(uint32_t windowID) {
    return CFBridgingRelease(windowSpaces(connection, 7, (__bridge CFArrayRef)@[@(windowID)])) ?: @[];
}
static NSDictionary *windowRow(uint32_t windowID) {
    NSArray *rows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, windowID));
    for (NSDictionary *row in rows) {
        if ([row[(id)kCGWindowNumber] unsignedIntValue] == windowID) return row;
    }
    return nil;
}
static Class moveClass(void) {
    Class cls = NSClassFromString(@"SLSBridgedMoveWindowsToManagedSpaceOperation");
    if (![cls instancesRespondToSelector:NSSelectorFromString(@"initWithWindows:spaceID:")] ||
        ![cls instancesRespondToSelector:NSSelectorFromString(@"performWithWMBridgeDelegate")]) return Nil;
    return cls;
}
static void listSpaces(void) {
    NSMutableArray *result = [NSMutableArray array];
    for (NSDictionary *row in displays()) {
        NSMutableArray *spaces = [NSMutableArray array];
        NSNumber *current = row[@"Current Space"][@"ManagedSpaceID"] ?: @0;
        for (NSDictionary *space in row[@"Spaces"]) {
            NSNumber *sid = space[@"ManagedSpaceID"], *type = space[@"type"];
            if (sid && type) [spaces addObject:@{
                @"id": sid, @"type": type, @"is_visible": boolean([sid isEqual:current])
            }];
        }
        [result addObject:@{
            @"id": row[@"Display Identifier"] ?: @"", @"current_space": current, @"spaces": spaces
        }];
    }
    emit(@{@"displays": result, @"move_window_available": boolean(moveClass() != Nil)});
}
static void submitMove(uint32_t windowID, NSDictionary *expected, NSNumber *destination) {
    Class cls = moveClass();
    if (!cls) fail(@"background Space move routing is unavailable on this system");
    id operation = ((id (*)(id, SEL, id, uint64_t))objc_msgSend)(
        [cls alloc], NSSelectorFromString(@"initWithWindows:spaceID:"), @[@(windowID)], destination.unsignedLongLongValue);
    if (!operation) fail(@"cannot prepare Space move");
    // Recheck the destination and foreground immediately before requesting the move.
    if (!inactiveUserSpace(displays(), destination) ||
        NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == [expected[@"owner_pid"] intValue])
        fail(@"Space or foreground changed; observe before retrying");
    ((void (*)(id, SEL))objc_msgSend)(operation, NSSelectorFromString(@"performWithWMBridgeDelegate"));
}
static void reportMove(uint32_t windowID, NSDictionary *expected, NSNumber *destination,
                       NSArray *before, NSArray *beforeDisplays, pid_t frontBefore) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    NSArray *after = membership(windowID);
    while (![after isEqual:@[destination]] && deadline.timeIntervalSinceNow > 0) {
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        after = membership(windowID);
    }
    NSArray *visibleBefore = visibleSpaces(beforeDisplays), *visibleAfter = visibleSpaces(displays());
    pid_t frontAfter = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    NSDictionary *row = windowRow(windowID);
    BOOL confirmed = [after isEqual:@[destination]] &&
        [row[(id)kCGWindowOwnerPID] isEqual:expected[@"owner_pid"]];
    emit(@{
        @"window_id": @(windowID), @"space_id": destination,
        @"status": confirmed ? @"confirmed" : @"sent_unverified",
        @"before_spaces": before, @"after_spaces": after,
        @"window_changed": boolean(!catalogIdentityMatches(row, expected)),
        @"frontmost_pid_before": @(frontBefore), @"frontmost_pid_after": @(frontAfter),
        @"foreground_changed": boolean(frontBefore != frontAfter),
        @"visible_spaces_before": visibleBefore, @"visible_spaces_after": visibleAfter,
        @"visible_spaces_changed": boolean(![visibleBefore isEqual:visibleAfter])
    });
}
static void moveWindow(NSDictionary *request, NSDictionary *expected, uint32_t windowID) {
    NSNumber *destination = request[@"space_id"];
    NSNumber *issuedAt = request[@"issued_at"];
    pid_t caller = [request[@"caller_pid"] intValue];
    NSString *lockPath = request[@"lock_path"];
    if (![destination isKindOfClass:NSNumber.class] || destination.unsignedLongLongValue == 0 ||
        ![issuedAt isKindOfClass:NSNumber.class] || caller <= 0 ||
        ![lockPath isKindOfClass:NSString.class]) fail(@"invalid Space move request");
    int fd = open(lockPath.fileSystemRepresentation, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (fd < 0) fail(@"cannot open input lock");
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); fail(@"another input action is running for this app"); }
    NSArray *beforeDisplays = displays();
    if (!inactiveUserSpace(beforeDisplays, destination)) fail(@"destination must be an existing inactive user Space");
    NSArray *before = membership(windowID);
    if (before.count == 0) fail(@"window Space membership is unavailable");
    pid_t frontBefore = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    NSTimeInterval age = NSDate.date.timeIntervalSince1970 - issuedAt.doubleValue;
    if (age < 0 || age > 300 || kill(caller, 0) != 0) fail(@"move target expired or caller exited");
    if (frontBefore == [expected[@"owner_pid"] intValue]) fail(@"target app is in the foreground");
    if (!catalogIdentityMatches(windowRow(windowID), expected)) fail(@"window changed; take a new snapshot");
    if (![before isEqual:@[destination]]) submitMove(windowID, expected, destination);
    reportMove(windowID, expected, destination, before, beforeDisplays, frontBefore);
    close(fd);
}

int main(void) { @autoreleasepool {
    alarm(20);
    NSData *data = NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
    NSDictionary *request = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (![request isKindOfClass:NSDictionary.class]) fail(@"invalid Space request");
    void *library = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY | RTLD_GLOBAL);
    Connection getConnection = library ? dlsym(library, "SLSMainConnectionID") : NULL;
    displaySpaces = library ? dlsym(library, "SLSCopyManagedDisplaySpaces") : NULL;
    windowSpaces = library ? dlsym(library, "SLSCopySpacesForWindows") : NULL;
    if (!getConnection || !displaySpaces || !windowSpaces) fail(@"Space catalog is unavailable on this system");
    connection = getConnection();
    NSString *action = request[@"action"];
    if ([action isEqual:@"list"]) { listSpaces(); return 0; }
    NSDictionary *expected = request[@"window"];
    uint32_t windowID = [expected[@"window_id"] unsignedIntValue];
    if (!windowID || !catalogIdentityMatches(windowRow(windowID), expected)) fail(@"window changed; observe again");
    if ([action isEqual:@"membership"]) {
        NSArray *spaces = membership(windowID);
        if (!catalogIdentityMatches(windowRow(windowID), expected)) fail(@"window changed during Space lookup");
        emit(@{@"window_id": @(windowID), @"space_ids": spaces});
    } else if ([action isEqual:@"move"]) { moveWindow(request, expected, windowID); }
    else fail(@"unsupported Space action");
    return 0;
} }
