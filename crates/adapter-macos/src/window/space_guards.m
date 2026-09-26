#import <Cocoa/Cocoa.h>

static NSArray *visibleSpaces(NSArray *displays) {
    NSMutableSet *visible = [NSMutableSet set];
    for (NSDictionary *display in displays) {
        NSNumber *current = display[@"Current Space"][@"ManagedSpaceID"];
        if (current) [visible addObject:current];
    }
    return [[visible allObjects] sortedArrayUsingSelector:@selector(compare:)];
}

static BOOL inactiveUserSpace(NSArray *displays, NSNumber *destination) {
    if ([visibleSpaces(displays) containsObject:destination]) return NO;
    for (NSDictionary *display in displays) {
        for (NSDictionary *space in display[@"Spaces"]) {
            if ([space[@"ManagedSpaceID"] isEqual:destination] &&
                space[@"type"] && [space[@"type"] intValue] == 0) return YES;
        }
    }
    return NO;
}

static BOOL catalogIdentityMatches(NSDictionary *row, NSDictionary *expected) {
    CGRect frame;
    NSDictionary *bounds = expected[@"bounds"];
    NSDictionary *raw = row[(id)kCGWindowBounds];
    if (!row || !bounds || ![raw isKindOfClass:NSDictionary.class] || !CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)raw, &frame)) return NO;
    return [row[(id)kCGWindowNumber] isEqual:expected[@"window_id"]] &&
        [row[(id)kCGWindowOwnerPID] isEqual:expected[@"owner_pid"]] &&
        [row[(id)kCGWindowName] isEqual:expected[@"title"]] &&
        (NSInteger)frame.origin.x == [bounds[@"x"] integerValue] &&
        (NSInteger)frame.origin.y == [bounds[@"y"] integerValue] &&
        (NSInteger)frame.size.width == [bounds[@"width"] integerValue] &&
        (NSInteger)frame.size.height == [bounds[@"height"] integerValue];
}
