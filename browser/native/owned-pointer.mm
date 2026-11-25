/** Native pointer delivery uses the exact responder proven by browser-owned WebContents focus. */
#include <node_api.h>
#include <cmath>
#include <cstring>
#import <AppKit/AppKit.h>
static const size_t argumentCount=5,kindBytes=16,maxFocusTokens=128;
@interface OyaPointerFocus : NSObject
@property(weak) NSWindow *window;
@property(weak) NSResponder *responder;
@end
@implementation OyaPointerFocus
@end
static NSMutableDictionary<NSString *,OyaPointerFocus *> *focusTokens;
/** Invalid native input fails without falling back to DOM events or a different native view. */
static void fail(napi_env env,const char*message){napi_throw_error(env,nullptr,message);}
/** Caller-owned handles originate only from the verified live Electron window on its UI thread. */
static NSView* rootView(napi_env env,napi_value value){
  void*bytes=nullptr;size_t size=0;uintptr_t address=0;
  if(!NSThread.isMainThread||napi_get_buffer_info(env,value,&bytes,&size)!=napi_ok||size!=sizeof(address)){fail(env,"Invalid native window handle");return nil;}
  std::memcpy(&address,bytes,sizeof(address));NSView*root=(__bridge NSView*)reinterpret_cast<void*>(address);
  if(!root||![root isKindOfClass:NSView.class]){fail(env,"Native handle is not a view");return nil;}return root;
}
/** Coordinate and bounds validation cannot invoke page-selected native targets. */
static double number(napi_env env,napi_value object,const char*key){
  napi_value value;double result=NAN;
  if(napi_get_named_property(env,object,key,&value)!=napi_ok||napi_get_value_double(env,value,&result)!=napi_ok||!std::isfinite(result))fail(env,"Invalid owned view bounds");
  return result;
}
/** Tokens retain neither a personal responder nor a window beyond this synchronous attempt. */
static napi_value captureFocus(napi_env env,napi_callback_info info){
  size_t count=1;napi_value args[1],result;napi_get_cb_info(env,info,&count,args,nullptr,nullptr);
  NSView*root=count==1?rootView(env,args[0]):nil;if(!root)return nullptr;
  if(!focusTokens)focusTokens=[NSMutableDictionary new];
  if(focusTokens.count>=maxFocusTokens){fail(env,"Native focus capacity exceeded");return nullptr;}
  OyaPointerFocus*focus=[OyaPointerFocus new];focus.window=root.window;focus.responder=root.window.firstResponder;
  NSString*token=NSUUID.UUID.UUIDString;focusTokens[token]=focus;
  napi_create_string_utf8(env,token.UTF8String,NAPI_AUTO_LENGTH,&result);return result;
}
/** Consume every token; takeover and destruction forbid restoring stale native focus. */
static napi_value finishFocus(napi_env env,napi_callback_info info){
  size_t count=2;napi_value args[2],result;char token[64]={};size_t length=0;bool restore=false;
  napi_get_cb_info(env,info,&count,args,nullptr,nullptr);
  if(count!=2||!NSThread.isMainThread||napi_get_value_string_utf8(env,args[0],token,sizeof(token),&length)!=napi_ok||length>=sizeof(token)||napi_get_value_bool(env,args[1],&restore)!=napi_ok){fail(env,"Invalid native focus token");return nullptr;}
  OyaPointerFocus*focus=focusTokens[[NSString stringWithUTF8String:token]];[focusTokens removeObjectForKey:[NSString stringWithUTF8String:token]];
  NSWindow*window=focus.window;NSResponder*prior=focus.responder;bool restored=false;
  if(restore&&window&&window.visible&&window.keyWindow){
    if([prior isKindOfClass:NSView.class]&&[(NSView*)prior window]!=window)prior=nil;
    restored=[window makeFirstResponder:prior];if(!restored)[window makeFirstResponder:nil];
  }
  napi_get_boolean(env,restored,&result);return result;
}
/** Only the proven first responder receives events; root hit testing could select the shield or a personal tab. */
static NSView* recipient(NSView*root,NSPoint point){
  NSWindow*window=root.window;if(!window||!window.visible||window.miniaturized||!window.keyWindow||window.attachedSheet)return nil;
  NSResponder*focused=window.firstResponder;if(![focused isKindOfClass:NSView.class])return nil;
  NSView*target=(NSView*)focused;if(target.window!=window||![target isDescendantOf:root])return nil;
  return NSPointInRect([target convertPoint:point fromView:root],target.bounds)?target:nil;
}
/** Genuine AppKit events enter Chrome's native widget and its own frame hit testing. */
static void deliver(NSView*root,NSView*target,NSPoint point,NSEventType type){
  NSEvent*event=[NSEvent mouseEventWithType:type location:[root convertPoint:point toView:nil] modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:root.window.windowNumber context:nil eventNumber:0 clickCount:type==NSEventTypeMouseMoved?0:1 pressure:type==NSEventTypeLeftMouseDown?1:0];
  if(type==NSEventTypeMouseMoved)[target mouseMoved:event];else if(type==NSEventTypeLeftMouseDown)[target mouseDown:event];else[target mouseUp:event];
}
/** Fixed event names travel back to the caller even when release was only best effort. */
static napi_value eventList(napi_env env,NSArray<NSString*>*events){
  napi_value result;napi_create_array_with_length(env,events.count,&result);
  for(NSUInteger i=0;i<events.count;i++){napi_value value;napi_create_string_utf8(env,events[i].UTF8String,NAPI_AUTO_LENGTH,&value);napi_set_element(env,result,i,value);}return result;
}
/** Partial native delivery is an error carrying the exact attempted event sequence. */
static napi_value inputFailure(napi_env env,NSArray<NSString*>*events){
  napi_value message,error;napi_create_string_utf8(env,"Native dispatch failed; input may have been sent. Inspect before retrying.",NAPI_AUTO_LENGTH,&message);napi_create_error(env,nullptr,message,&error);
  napi_set_named_property(env,error,"dispatched",eventList(env,events));napi_throw(env,error);return nullptr;
}
/** One native call holds the recipient through both buttons, including cancellation-safe release. */
static napi_value sendMouse(napi_env env,napi_callback_info info){
  size_t count=argumentCount;napi_value args[argumentCount];napi_get_cb_info(env,info,&count,args,nullptr,nullptr);
  if(count!=argumentCount){fail(env,"Invalid native mouse arguments");return nullptr;}NSView*root=rootView(env,args[0]);if(!root)return nullptr;
  const double bx=number(env,args[1],"x"),by=number(env,args[1],"y"),width=number(env,args[1],"width"),height=number(env,args[1],"height");
  double x=NAN,y=NAN;napi_get_value_double(env,args[2],&x);napi_get_value_double(env,args[3],&y);
  if(!std::isfinite(bx)||!std::isfinite(by)||!std::isfinite(width)||!std::isfinite(height)||width<=0||height<=0||!std::isfinite(x)||!std::isfinite(y)||x<0||y<0||x>=width||y>=height){fail(env,"Native point is outside the owned view");return nullptr;}
  char kind[kindBytes]={};size_t length=0;
  if(napi_get_value_string_utf8(env,args[4],kind,sizeof(kind),&length)!=napi_ok||length>=sizeof(kind)){fail(env,"Unsupported native mouse event");return nullptr;}
  const bool move=std::strcmp(kind,"mouseMove")==0,pair=std::strcmp(kind,"mouseClick")==0;
  if(!move&&!pair){fail(env,"Unsupported native mouse event");return nullptr;}
  NSPoint point=NSMakePoint(NSMinX(root.bounds)+bx+x,root.isFlipped?NSMinY(root.bounds)+by+y:NSMaxY(root.bounds)-by-y);
  __attribute__((objc_precise_lifetime)) NSView*target=recipient(root,point);
  if(!target){fail(env,"Owned-page native responder proof failed. No pointer input was sent.");return nullptr;}

  NSMutableArray*events=[NSMutableArray new];
  @try{
    if(move){[events addObject:@"mousemove"];deliver(root,target,point,NSEventTypeMouseMoved);}
    else{[events addObject:@"mousedown"];deliver(root,target,point,NSEventTypeLeftMouseDown);[events addObject:@"mouseup"];deliver(root,target,point,NSEventTypeLeftMouseUp);}
  }@catch(NSException*exception){
    if([events containsObject:@"mousedown"]&&![events containsObject:@"mouseup"]){@try{[events addObject:@"mouseup_cleanup"];deliver(root,target,point,NSEventTypeLeftMouseUp);}@catch(NSException*ignored){}}
    return inputFailure(env,events);
  }return eventList(env,events);
}
/** The application-only addon exposes no arbitrary native selector or handle to page JavaScript. */
static napi_value initialize(napi_env env,napi_value exports){
  struct Entry{const char*name;napi_callback callback;};Entry entries[]={{"captureFocus",captureFocus},{"finishFocus",finishFocus},{"sendMouse",sendMouse}};
  for(const auto&entry:entries){napi_value function;napi_create_function(env,entry.name,NAPI_AUTO_LENGTH,entry.callback,nullptr,&function);napi_set_named_property(env,exports,entry.name,function);}return exports;
}
NAPI_MODULE(NODE_GYP_MODULE_NAME,initialize)
