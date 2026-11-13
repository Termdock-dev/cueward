#undef main

static void requireCatalog(BOOL result, const char *message) {
    if (!result) { fprintf(stderr, "%s\n", message); exit(1); }
}
int main(void) { @autoreleasepool {
    NSDictionary *original = @{
        (id)kCGWindowNumber: @7, (id)kCGWindowOwnerPID: @123, (id)kCGWindowName: @"Save",
        (id)kCGWindowBounds: CFBridgingRelease(CGRectCreateDictionaryRepresentation(CGRectMake(-100, 200, 640, 480)))
    };
    NSDictionary *expected = @{@"window_id": @7, @"owner_pid": @123, @"title": @"Save",
        @"bounds": @{@"x": @(-100), @"y": @200, @"width": @640, @"height": @480}};
    NSMutableDictionary *bystander = [original mutableCopy];
    bystander[(id)kCGWindowNumber] = @8;
    fullCatalog = @[bystander, original];
    individualCatalog = @[];
    requireCatalog(catalogIdentityMatches(windowRow(7), expected), "remote window absent from individual lookup was rejected");
    for (id key in @[(id)kCGWindowNumber, (id)kCGWindowOwnerPID, (id)kCGWindowName, (id)kCGWindowBounds]) {
        NSMutableDictionary *changed = [original mutableCopy];
        changed[key] = [key isEqual:(id)kCGWindowBounds]
            ? CFBridgingRelease(CGRectCreateDictionaryRepresentation(CGRectMake(-101, 200, 640, 480))) : @999;
        fullCatalog = @[changed];
        requireCatalog(!catalogIdentityMatches(windowRow(7), expected), "changed remote identity accepted");
    }
    for (NSArray *rows in @[@[], @[original, original]]) {
        fullCatalog = rows;
        requireCatalog(windowRow(7) == nil, "missing or ambiguous remote window accepted");
    }
    fullCatalog = @[original];
    catalogUnavailable = YES;
    requireCatalog(windowRow(7) == nil, "unavailable full catalog accepted");
    catalogUnavailable = NO;
    individualCatalog = @[bystander];
    requireCatalog(catalogIdentityMatches(windowRow(7), expected), "unrelated individual row hid the current window");
    NSMutableDictionary *conflict = [original mutableCopy];
    conflict[(id)kCGWindowOwnerPID] = @999;
    individualCatalog = @[conflict];
    requireCatalog(!catalogIdentityMatches(windowRow(7), expected), "full catalog overrode a conflicting individual identity");
    individualCatalog = @[original, original];
    requireCatalog(windowRow(7) == nil, "ambiguous individual window accepted");
    puts("passed");
} return 0; }
