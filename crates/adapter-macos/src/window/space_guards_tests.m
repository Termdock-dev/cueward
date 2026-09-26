static void require(BOOL result, const char *message) {
    if (!result) { fprintf(stderr, "%s\n", message); exit(1); }
}
int main(void) { @autoreleasepool {
    NSArray *catalog = @[
        @{@"Current Space": @{@"ManagedSpaceID": @1}, @"Spaces": @[
            @{@"ManagedSpaceID": @1, @"type": @0}, @{@"ManagedSpaceID": @2, @"type": @0},
            @{@"ManagedSpaceID": @3, @"type": @1}, @{@"ManagedSpaceID": @4}]},
        @{@"Current Space": @{@"ManagedSpaceID": @5}, @"Spaces": @[@{@"ManagedSpaceID": @5, @"type": @0}]}
    ];
    require(inactiveUserSpace(catalog, @2), "inactive user Space rejected");
    for (NSNumber *sid in @[@1, @3, @4, @5, @99]) require(!inactiveUserSpace(catalog, sid), "unsafe destination accepted");
    require([visibleSpaces(catalog) isEqual:@[@1, @5]], "visible Spaces lost");
    NSDictionary *unknownDisplay = @{@"Spaces": @[@{@"ManagedSpaceID": @2, @"type": @0}]};
    require(!inactiveUserSpace(@[unknownDisplay], @2), "unknown current Space accepted as inactive");
    for (id current in @[@{}, @{@"ManagedSpaceID": @"2"}, @{@"ManagedSpaceID": @0},
                         @{@"ManagedSpaceID": @YES}, @{@"ManagedSpaceID": @1.5}, NSNull.null]) {
        NSDictionary *display = @{@"Current Space": current, @"Spaces": unknownDisplay[@"Spaces"]};
        require(!inactiveUserSpace(@[display], @2), "malformed current Space accepted");
        require(!inactiveUserSpace(@[catalog[0], display], @2), "partial visibility accepted");
    }
    NSDictionary *expected = @{@"window_id": @7, @"owner_pid": @123, @"title": @"Fixture",
        @"bounds": @{@"x": @(-100), @"y": @200, @"width": @640, @"height": @480}};
    NSMutableDictionary *row = [@{
        (id)kCGWindowNumber: @7, (id)kCGWindowOwnerPID: @123, (id)kCGWindowName: @"Fixture",
        (id)kCGWindowBounds: CFBridgingRelease(CGRectCreateDictionaryRepresentation(CGRectMake(-100.5, 200.75, 640.5, 480.25)))
    } mutableCopy];
    require(catalogIdentityMatches(row, expected), "fractional identity rejected");
    for (id key in @[(id)kCGWindowNumber, (id)kCGWindowOwnerPID, (id)kCGWindowName]) {
        id original = row[key]; row[key] = @999;
        require(!catalogIdentityMatches(row, expected), "changed identity accepted"); row[key] = original;
    }
    row[(id)kCGWindowBounds] = CFBridgingRelease(CGRectCreateDictionaryRepresentation(CGRectMake(-101, 200, 640, 480)));
    require(!catalogIdentityMatches(row, expected), "moved frame accepted");
    require(!catalogIdentityMatches(@{}, expected), "missing frame accepted");
    puts("passed");
} return 0; }
