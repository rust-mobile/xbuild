@import Flutter;
@import UIKit;

@interface AppDelegate: FlutterAppDelegate
@end

@implementation AppDelegate

- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
    self.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
    FlutterViewController *flutterViewController =
        [[FlutterViewController alloc] initWithProject:nil nibName:nil bundle:nil];
    self.window.rootViewController = flutterViewController;
    [self.window makeKeyAndVisible];
    return YES;
}

@end

int main(int argc, const char * argv[]) {
    return UIApplicationMain(argc, argv, nil, @"AppDelegate");
}
