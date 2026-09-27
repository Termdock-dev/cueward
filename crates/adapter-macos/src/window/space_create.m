// Typed init messaging preserves ARC's consumed-self convention, including a nil result.
@protocol CuewardSpaceCreation <NSObject>
- (instancetype)initWithOptions:(unsigned int)options values:(NSDictionary *)values;
- (id)performWithWMBridgeDelegate;
@end
@protocol CuewardCreatedSpace <NSObject>
- (uint64_t)spaceID;
@end

static Class createClass(void) {
    Class cls = NSClassFromString(@"SLSBridgedSpaceCreateOperation");
    Class result = NSClassFromString(@"SLSBridgedWindowManagementOperationSpaceIDResult");
    if (![cls instancesRespondToSelector:NSSelectorFromString(@"initWithOptions:values:")] ||
        ![cls instancesRespondToSelector:NSSelectorFromString(@"performWithWMBridgeDelegate")] ||
        ![result instancesRespondToSelector:NSSelectorFromString(@"spaceID")]) return Nil;
    return cls;
}

static NSArray *creationCatalog(void) {
    id rows = CFBridgingRelease(displaySpaces(connection));
    return [rows isKindOfClass:NSArray.class] && visibleSpaces(rows) ? rows : nil;
}

static NSSet *creationSpaceIDs(NSArray *rows) {
    if (!rows.count) return nil;
    NSMutableSet *ids = [NSMutableSet set];
    for (NSDictionary *display in rows) {
        if (![display[@"Spaces"] isKindOfClass:NSArray.class]) return nil;
        NSMutableSet *local = [NSMutableSet set];
        for (id raw in display[@"Spaces"]) {
            if (![raw isKindOfClass:NSDictionary.class]) return nil;
            NSNumber *sid = raw[@"ManagedSpaceID"];
            if (!orderedMembership(@[sid ?: NSNull.null]) || [ids containsObject:sid]) return nil;
            [ids addObject:sid]; [local addObject:sid];
        }
        if (![local containsObject:display[@"Current Space"][@"ManagedSpaceID"]]) return nil;
    }
    return ids;
}

static NSString *createdSpaceDisplay(NSArray *rows, NSNumber *sid, NSString *uuid) {
    if (!creationSpaceIDs(rows) || [visibleSpaces(rows) containsObject:sid]) return nil;
    NSString *match = nil;
    for (NSDictionary *display in rows) for (NSDictionary *space in display[@"Spaces"]) {
        if (![space[@"ManagedSpaceID"] isEqual:sid]) continue;
        NSString *displayID = display[@"Display Identifier"];
        if (match || ![displayID isKindOfClass:NSString.class] || !displayID.length ||
            ![space[@"type"] isKindOfClass:NSNumber.class] ||
            CFGetTypeID((__bridge CFTypeRef)space[@"type"]) == CFBooleanGetTypeID() ||
            ![space[@"type"] isEqual:@0] || ![space[@"uuid"] isEqual:uuid]) return nil;
        match = displayID;
    }
    return match;
}

static void reportCreatedSpace(uint64_t candidate, NSSet *previous, NSString *uuid,
                               NSArray *before, pid_t frontBefore) {
    NSNumber *sid = candidate && ![previous containsObject:@(candidate)] ? @(candidate) : nil;
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    NSArray *after = creationCatalog();
    NSString *displayID = sid ? createdSpaceDisplay(after, sid, uuid) : nil;
    while (sid && !displayID && deadline.timeIntervalSinceNow > 0) {
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        after = creationCatalog();
        displayID = createdSpaceDisplay(after, sid, uuid);
    }
    NSArray *visibleBefore = visibleSpaces(before), *visibleAfter = visibleSpaces(after);
    pid_t frontAfter = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    emit(@{
        @"status": displayID ? @"confirmed" : @"sent_unverified",
        @"space_id": sid ?: NSNull.null, @"display_id": displayID ?: NSNull.null,
        @"reason": displayID ? NSNull.null : @"new inactive desktop was not confirmed; inspect Spaces before retrying",
        @"frontmost_pid_before": @(frontBefore), @"frontmost_pid_after": @(frontAfter),
        @"foreground_changed": boolean(frontBefore != frontAfter),
        @"visible_spaces_before": visibleBefore, @"visible_spaces_after": visibleAfter ?: NSNull.null,
        @"visible_spaces_changed": visibleAfter ? boolean(![visibleBefore isEqual:visibleAfter]) : NSNull.null,
    });
}

static void createSpace(NSDictionary *request) {
    NSString *path = request[@"lock_path"];
    pid_t caller = [request[@"caller_pid"] intValue];
    if (![path isKindOfClass:NSString.class] || caller <= 0) fail(@"invalid Space creation request");
    int fd = open(path.fileSystemRepresentation, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (fd < 0) fail(@"cannot open Space creation lock");
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); fail(@"another Space creation is running"); }
    NSArray *before = creationCatalog();
    NSSet *previous = creationSpaceIDs(before);
    pid_t front = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    Class cls = createClass();
    if (!previous || front <= 0) fail(@"Space catalog or foreground is unavailable; creation stopped");
    if (!cls) fail(@"native Space creation is unavailable on this system");
    if (kill(caller, 0) != 0) fail(@"Space creation caller exited");
    NSString *uuid = NSUUID.UUID.UUIDString;
    id<CuewardSpaceCreation> operation = [(id<CuewardSpaceCreation>)[cls alloc]
        initWithOptions:0 values:@{@"type": @0, @"uuid": uuid}];
    if (!operation) fail(@"cannot prepare native Space creation");
    if (kill(caller, 0) != 0) fail(@"Space creation caller exited before submission");
    id<CuewardCreatedSpace> result = [operation performWithWMBridgeDelegate];
    SEL selector = NSSelectorFromString(@"spaceID");
    uint64_t sid = [result respondsToSelector:selector] ? [result spaceID] : 0;
    reportCreatedSpace(sid, previous, uuid, before, front);
    close(fd);
}
