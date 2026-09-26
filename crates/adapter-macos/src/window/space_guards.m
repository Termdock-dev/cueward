#import <Cocoa/Cocoa.h>

static NSArray *visibleSpaces(NSArray *displays) {
    if (!displays.count) return nil;
    NSMutableSet *visible = [NSMutableSet set];
    for (NSDictionary *display in displays) {
        if (![display isKindOfClass:NSDictionary.class]) return nil;
        NSDictionary *currentSpace = display[@"Current Space"];
        if (![currentSpace isKindOfClass:NSDictionary.class]) return nil;
        NSNumber *current = currentSpace[@"ManagedSpaceID"];
        if (![current isKindOfClass:NSNumber.class] ||
            CFGetTypeID((__bridge CFTypeRef)current) == CFBooleanGetTypeID() ||
            !(current.doubleValue > 0) ||
            [current compare:@(current.unsignedLongLongValue)] != NSOrderedSame) return nil;
        [visible addObject:current];
    }
    return [[visible allObjects] sortedArrayUsingSelector:@selector(compare:)];
}

static BOOL inactiveUserSpace(NSArray *displays, NSNumber *destination) {
    NSArray *visible = visibleSpaces(displays);
    if (!visible || [visible containsObject:destination]) return NO;
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
