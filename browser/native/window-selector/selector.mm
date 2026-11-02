/** Main-process-only single-window picker; never creates a stream or observes global input. */
#import <Cocoa/Cocoa.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#include <node_api.h>
#include <memory>
#include <cmath>

/** Native window identity includes the owning process lifetime, never the display title. */
struct Choice {
  /** Exact CGWindowID selected by Apple's picker. */
  uint32_t window = 0;
  /** Native owning process, independent of the display title. */
  int32_t process = 0;
  /** Process launch time fences PID reuse. */
  double launched = 0;
  /** Fixed terminal state; native errors never leak their descriptions. */
  const char *status = "cancelled";
};
struct Runtime;
/** One observer remains owned until a terminal callback, including after cancellation. */
@interface OyaWindowPicker : NSObject <SCContentSharingPickerObserver>
@property(nonatomic, assign) Runtime *runtime;
@end
/** Node owns one process-wide session; callbacks carry no reference to a renderer or site. */
struct Runtime {
  /** The only Node environment permitted to own this process-wide picker. */
  napi_env env;
  /** One bounded result callback, unreferenced so it cannot keep shutdown alive. */
  napi_threadsafe_function callback = nullptr;
  /** Retained until a native terminal result, including after requested cancellation. */
  __strong OyaWindowPicker *observer = nil;
  /** Abandoned requests never deliver a successful choice. */
  bool revoked = false;
};
/** Only one Node environment may own Apple's process-wide picker. */
static Runtime *owner = nullptr;
/** Bound exclusions to helper windows supplied by Oya main, never arbitrary page data. */
static constexpr uint32_t maxExcludedWindows = 128;

/** Boolean results distinguish native admission from eventual selection. */
static napi_value boolean(napi_env env, bool value) {
  napi_value result;
  napi_get_boolean(env, value, &result);
  return result;
}
/** Fixed metadata fields cannot expose native handles or credential-like data. */
static void number(napi_env env, napi_value object, const char *name, double value) {
  napi_value field;
  napi_create_double(env, value, &field);
  napi_set_named_property(env, object, name, field);
}
/** Thread-safe callback cleanup also frees a queued result after Node has stopped accepting callbacks. */
static void deliver(napi_env env, napi_value callback, void *, void *data) {
  std::unique_ptr<Choice> choice(static_cast<Choice *>(data));
  if (!env || !callback) return;
  napi_value result, status, receiver;
  napi_create_object(env, &result);
  napi_create_string_utf8(env, choice->status, NAPI_AUTO_LENGTH, &status);
  napi_set_named_property(env, result, "status", status);
  number(env, result, "windowId", choice->window);
  number(env, result, "processId", choice->process);
  number(env, result, "processStarted", choice->launched);
  napi_get_undefined(env, &receiver);
  napi_call_function(env, receiver, callback, 1, &result, nullptr);
}
/** Terminal callbacks release native ownership before delivering one result to the original caller. */
static void finish(Runtime *runtime, Choice choice) {
  if (!runtime || !runtime->observer) return;
  if (runtime->revoked) choice = Choice();
  if (@available(macOS 15.2, *)) {
    [SCContentSharingPicker.sharedPicker removeObserver:runtime->observer];
    SCContentSharingPicker.sharedPicker.active = NO;
  }
  runtime->observer.runtime = nullptr;
  runtime->observer = nil;
  auto callback = runtime->callback;
  runtime->callback = nullptr;
  auto result = new Choice(choice);
  if (napi_call_threadsafe_function(callback, result, napi_tsfn_nonblocking) != napi_ok) delete result;
  napi_release_threadsafe_function(callback, napi_tsfn_release);
}
/** A process restart cannot inherit a prior window selection just by reusing its PID. */
static double launchTime(pid_t process) {
  NSRunningApplication *application = [NSRunningApplication runningApplicationWithProcessIdentifier:process];
  return application && !application.terminated ? application.launchDate.timeIntervalSince1970 : 0;
}
/** Native filter shape and exact process identity must agree before Oya can offer final confirmation. */
static Choice selected(SCContentFilter *filter) {
  Choice choice;
  choice.status = "unavailable";
  if (@available(macOS 15.2, *)) {
    if (filter.style != SCShareableContentStyleWindow || filter.includedWindows.count != 1) return choice;
    SCWindow *window = filter.includedWindows.firstObject;
    choice.window = window.windowID;
    choice.process = window.owningApplication.processID;
    choice.launched = launchTime(choice.process);
    if (choice.window && choice.process > 0 && choice.launched > 0) choice.status = "selected";
  }
  return choice;
}
@implementation OyaWindowPicker
/** A nil stream is our pre-capture picker, never an unrelated active stream's settings. */
- (void)contentSharingPicker:(SCContentSharingPicker *)picker didCancelForStream:(SCStream *)stream API_AVAILABLE(macos(14.0)) {
  if (!stream) dispatch_async(dispatch_get_main_queue(), ^{ finish(self.runtime, Choice()); });
}
/** First selection identifies a candidate only; Oya's later confirmation owns capture. */
- (void)contentSharingPicker:(SCContentSharingPicker *)picker didUpdateWithFilter:(SCContentFilter *)filter forStream:(SCStream *)stream API_AVAILABLE(macos(14.0)) {
  if (!stream) dispatch_async(dispatch_get_main_queue(), ^{ finish(self.runtime, selected(filter)); });
}
/** Failure is terminal and contains no OS error text or private source metadata. */
- (void)contentSharingPickerStartDidFailWithError:(NSError *)error {
  dispatch_async(dispatch_get_main_queue(), ^{
    Choice choice;
    choice.status = "failed";
    finish(self.runtime, choice);
  });
}
@end

/** Validate the entire helper-window exclusion list before starting native UI. */
static NSArray<NSNumber *> *exclusions(napi_env env, napi_value value) {
  bool array = false;
  uint32_t length = 0;
  if (napi_is_array(env, value, &array) != napi_ok || !array) return nil;
  if (napi_get_array_length(env, value, &length) != napi_ok || length > maxExcludedWindows) return nil;
  NSMutableArray<NSNumber *> *result = [NSMutableArray new];
  for (uint32_t index = 0; index < length; index++) {
    napi_value item;
    double id = 0;
    if (napi_get_element(env, value, index, &item) != napi_ok || napi_get_value_double(env, item, &id) != napi_ok) return nil;
    if (!std::isfinite(id) || id <= 0 || id > UINT32_MAX || id != static_cast<uint32_t>(id)) return nil;
    [result addObject:@(static_cast<uint32_t>(id))];
  }
  return result;
}
/** A single bounded result queue never keeps browser shutdown alive on an abandoned OS picker. */
static bool makeCallback(Runtime *runtime, napi_value callback) {
  napi_value resource;
  napi_create_string_utf8(runtime->env, "OyaWindowSelection", NAPI_AUTO_LENGTH, &resource);
  auto status = napi_create_threadsafe_function(runtime->env, callback, nullptr, resource, 1, 1,
    nullptr, nullptr, nullptr, deliver, &runtime->callback);
  if (status != napi_ok) return false;
  napi_unref_threadsafe_function(runtime->env, runtime->callback);
  return true;
}
/** No global input hooks: Apple's single-window picker owns pointer handling and accessibility. */
static void present(Runtime *runtime, NSArray<NSNumber *> *excluded) {
  if (@available(macOS 15.2, *)) {
    runtime->revoked = false;
    runtime->observer = [OyaWindowPicker new];
    runtime->observer.runtime = runtime;
    auto picker = SCContentSharingPicker.sharedPicker;
    auto configuration = [SCContentSharingPickerConfiguration new];
    configuration.allowedPickerModes = SCContentSharingPickerModeSingleWindow;
    configuration.allowsChangingSelectedContent = NO;
    configuration.excludedWindowIDs = excluded;
    picker.defaultConfiguration = configuration;
    [picker addObserver:runtime->observer];
    picker.active = YES;
    [picker presentPickerUsingContentStyle:SCShareableContentStyleWindow];
  }
}
/** Unsupported hosts and unresolved prior sessions refuse without replacing their observer. */
static napi_value start(napi_env env, napi_callback_info info) {
  size_t count = 2;
  napi_value args[2];
  napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
  if (count != 2 || !owner || owner->env != env || owner->observer) return boolean(env, false);
  if (@available(macOS 15.2, *)) {
    auto excluded = exclusions(env, args[1]);
    if (!excluded || !makeCallback(owner, args[0])) return boolean(env, false);
    present(owner, excluded);
    return boolean(env, true);
  }
  return boolean(env, false);
}
/** Deactivation is not an acknowledgment: retain the observer until a real terminal callback. */
static napi_value cancel(napi_env env, napi_callback_info) {
  if (owner && owner->env == env && owner->observer) {
    owner->revoked = true;
    if (@available(macOS 15.2, *)) SCContentSharingPicker.sharedPicker.active = NO;
  }
  return boolean(env, true);
}
/** A quarantined cancelled session is unavailable until its old callback is consumed. */
static napi_value ready(napi_env env, napi_callback_info) {
  if (@available(macOS 15.2, *)) return boolean(env, owner && owner->env == env && !owner->observer);
  return boolean(env, false);
}
/** Native source identity is checked again before final capture without enumerating titles. */
static napi_value current(napi_env env, napi_callback_info info) {
  size_t count = 3;
  napi_value args[3];
  double window = 0, process = 0, launched = 0;
  napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
  if (count != 3 || napi_get_value_double(env, args[0], &window) != napi_ok || napi_get_value_double(env, args[1], &process) != napi_ok || napi_get_value_double(env, args[2], &launched) != napi_ok) return boolean(env, false);
  if (!std::isfinite(window) || !std::isfinite(process) || !std::isfinite(launched)) return boolean(env, false);
  if (window <= 0 || window > UINT32_MAX || process <= 0 || process > INT32_MAX || launched <= 0) return boolean(env, false);
  if (window != static_cast<uint32_t>(window) || process != static_cast<int32_t>(process)) return boolean(env, false);
  NSArray *list = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, static_cast<CGWindowID>(window)));
  NSDictionary *entry = [list count] == 1 ? [list firstObject] : nil;
  bool matches = [entry[(id)kCGWindowNumber] unsignedIntValue] == window && [entry[(id)kCGWindowOwnerPID] intValue] == process;
  return boolean(env, matches && launchTime(static_cast<pid_t>(process)) == launched);
}
/** Shutdown removes the observer before releasing Node callbacks; late native messages cannot reach a dead environment. */
static void cleanup(void *data) {
  auto runtime = static_cast<Runtime *>(data);
  runtime->observer.runtime = nullptr;
  if (@available(macOS 15.2, *)) if (runtime->observer) {
    [SCContentSharingPicker.sharedPicker removeObserver:runtime->observer];
    SCContentSharingPicker.sharedPicker.active = NO;
  }
  if (runtime->callback) napi_release_threadsafe_function(runtime->callback, napi_tsfn_abort);
  owner = nullptr;
  delete runtime;
}
/** Only Oya's Cocoa main thread may instantiate this in-process adapter. */
static napi_value initialize(napi_env env, napi_value exports) {
  if (![NSThread isMainThread] || owner) { napi_throw_error(env, nullptr, "Window selection requires the main process"); return nullptr; }
  owner = new Runtime{env};
  napi_add_env_cleanup_hook(env, cleanup, owner);
  napi_property_descriptor properties[] = {
    {"start", nullptr, start, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"cancel", nullptr, cancel, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"ready", nullptr, ready, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"current", nullptr, current, nullptr, nullptr, nullptr, napi_default, nullptr},
  };
  napi_define_properties(env, exports, sizeof(properties) / sizeof(properties[0]), properties);
  return exports;
}
NAPI_MODULE(NODE_GYP_MODULE_NAME, initialize)
