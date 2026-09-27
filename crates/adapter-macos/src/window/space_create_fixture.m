#import <Cocoa/Cocoa.h>
#import <objc/message.h>
#import <sys/file.h>
#import <fcntl.h>
#import <signal.h>

static NSDictionary *request;
static NSString *scenario;
static NSDictionary *submittedValues;
static NSUInteger submittedOptions, calls, afterReads;
static BOOL prepared;
static void trace(void) {
    NSDictionary *value = @{@"calls": @(calls), @"options": @(submittedOptions),
                            @"values": submittedValues ?: @{}, @"prepared": @(prepared)};
    [[NSJSONSerialization dataWithJSONObject:value options:0 error:nil] writeToFile:request[@"trace"] atomically:YES];
}
static NSNumber *boolean(BOOL value) { return value ? @YES : @NO; }
static void fail(NSString *message) { trace(); fprintf(stderr, "%s\n", message.UTF8String); exit(1); }
static void emit(id value) {
    trace(); NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    fwrite(data.bytes,1,data.length,stdout);
}

@interface CreatedResult : NSObject
- (uint64_t)spaceID;
@end
@implementation CreatedResult
- (uint64_t)spaceID { return [scenario isEqual:@"zero"] ? 0 : [scenario isEqual:@"reused"] ? 1 : 99; }
@end
@interface CreateOperation : NSObject
- (instancetype)initWithOptions:(unsigned int)options values:(NSDictionary *)values;
- (id)performWithWMBridgeDelegate;
@end
@implementation CreateOperation
- (instancetype)initWithOptions:(unsigned int)options values:(NSDictionary *)values {
    self = [super init]; prepared = YES; submittedOptions = options; submittedValues = values;
    return [scenario isEqual:@"prepare-failed"] ? nil : self;
}
- (id)performWithWMBridgeDelegate {
    calls++; trace();
    return [scenario isEqual:@"missing-result"] ? nil : [[CreatedResult alloc] init];
}
@end
static Class fixtureClass(NSString *name) {
    if ([scenario isEqual:@"unavailable"]) return Nil;
    return [name isEqual:@"SLSBridgedSpaceCreateOperation"] ? CreateOperation.class : CreatedResult.class;
}
@interface TestRunningApp : NSObject
@property pid_t processIdentifier;
@end
@implementation TestRunningApp
@end
@interface TestWorkspace : NSObject
+ (instancetype)sharedWorkspace;
- (TestRunningApp *)frontmostApplication;
@end
@implementation TestWorkspace
+ (instancetype)sharedWorkspace { return [[self alloc] init]; }
- (TestRunningApp *)frontmostApplication {
    TestRunningApp *app = [[TestRunningApp alloc] init];
    app.processIdentifier = [scenario isEqual:@"no-foreground"] ? 0 :
        calls && [scenario isEqual:@"foreground-changed"] ? 202 : 101;
    return app;
}
@end
static int testKill(pid_t pid, int sig) {
    if ([scenario isEqual:@"dead-caller"] || (prepared && [scenario isEqual:@"caller-exited"])) return -1;
    return kill(pid,sig);
}
static CFArrayRef testCatalog(int connection) {
    if ((calls && [scenario isEqual:@"missing-after"]) || [scenario isEqual:@"missing-before"]) return NULL;
    NSMutableArray *spaces = [NSMutableArray arrayWithObject:@{@"ManagedSpaceID":@1,@"type":@0}];
    NSNumber *current = @1;
    if (calls) {
        afterReads++;
        BOOL delayed = [scenario isEqual:@"delayed"] && afterReads < 3;
        if (!delayed && ![scenario isEqual:@"absent"]) {
            NSString *uuid = [scenario isEqual:@"wrong-uuid"] ? @"different" : submittedValues[@"uuid"];
            [spaces addObject:@{@"ManagedSpaceID":@99,@"type":[scenario isEqual:@"wrong-type"] ? @4 : @0,@"uuid":uuid}];
            if ([scenario isEqual:@"visible"]) current = @99;
        }
    }
    NSDictionary *display = @{@"Display Identifier":@"fixture-display",@"Current Space":@{@"ManagedSpaceID":current},@"Spaces":spaces};
    NSArray *rows = calls && [scenario isEqual:@"duplicate"] ? @[display,display] : @[display];
    return CFBridgingRetain(rows);
}
typedef CFArrayRef (*DisplaySpaces)(int);
static DisplaySpaces displaySpaces = testCatalog;
static int connection;
#define NSClassFromString fixtureClass
#define NSWorkspace TestWorkspace
#define kill testKill
