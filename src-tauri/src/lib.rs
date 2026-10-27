use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, State, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
mod notifications;
use base64::Engine;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics,SYSTEM_METRICS_INDEX};
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY};

struct MonitorState(Arc<Mutex<Option<(thread::JoinHandle<()>, Arc<AtomicBool>)>>>);

fn validate_threshold(threshold: f32) -> Result<(), String> {
 if !threshold.is_finite() || !(0.0..=254.0).contains(&threshold) {
  return Err("颜色通道变化阈值必须在 0–254 之间".into());
 }
 Ok(())
}

#[tauri::command]
async fn test_notification(app: AppHandle) -> Result<(), String> {
 tauri::async_runtime::spawn_blocking(move || notifications::send(&app, "这是一条测试通知，区域变化监测器的通知功能可用。"))
  .await.map_err(|e| e.to_string())?
}

fn capture(x:i32,y:i32,w:i32,h:i32)->Vec<u8>{ unsafe {
 let screen=GetDC(None); let dc=CreateCompatibleDC(Some(screen)); let bmp=CreateCompatibleBitmap(screen,w,h); SelectObject(dc,bmp.into()); BitBlt(dc,0,0,w,h,Some(screen),x,y,SRCCOPY);
 let mut info=BITMAPINFO{bmiHeader:BITMAPINFOHEADER{biSize:std::mem::size_of::<BITMAPINFOHEADER>() as u32,biWidth:w,biHeight:-h,biPlanes:1,biBitCount:24,biCompression:BI_RGB.0,..Default::default()},..Default::default()}; let stride=((w*3+3)/4)*4; let mut out=vec![0u8;(stride*h) as usize]; GetDIBits(dc,bmp,0,h as u32,Some(out.as_mut_ptr() as *mut _),&mut info,DIB_RGB_COLORS); DeleteObject(bmp.into()); DeleteDC(dc); ReleaseDC(None,screen); out }
}
#[tauri::command]
fn start_monitor(app:AppHandle,state:State<MonitorState>,x:i32,y:i32,w:i32,h:i32,interval_ms:u64,threshold:f32,duration_ms:u64)->Result<(),String>{
 validate_threshold(threshold)?;
 if w<2||h<2{return Err("区域尺寸无效".into())}; let mut slot=state.0.lock().unwrap(); if slot.is_some(){return Err("监测已在运行".into())}; let stop=Arc::new(AtomicBool::new(false)); let stop_thread=stop.clone(); let handle=thread::spawn(move||{let mut base=capture(x,y,w,h); let mut changed_since:Option<std::time::Instant>=None; while !stop_thread.load(Ordering::Relaxed) { thread::park_timeout(Duration::from_millis(interval_ms.max(100))); if stop_thread.load(Ordering::Relaxed){break} let now=capture(x,y,w,h); let different=base.iter().zip(now.iter()).filter(|(a,b)|(**a as i16-**b as i16).abs()>threshold as i16).count() as f32/base.len() as f32; if different>0.03 {if changed_since.is_none(){changed_since=Some(std::time::Instant::now())} if changed_since.unwrap().elapsed().as_millis()>=duration_ms as u128 {
  match notifications::send(&app, "检测区域发生持续变化") {
   Ok(()) => { let _ = app.emit("region-changed", ()); }
   Err(error) => { eprintln!("通知失败：{error}"); let _ = app.emit("notification-error", error); }
  }
  changed_since=None; base=now;}} else {changed_since=None;} }}); *slot=Some((handle,stop)); Ok(())
}
#[derive(Clone, serde::Serialize)]
struct ScreenShot { x:i32, y:i32, width:i32, height:i32, stride:i32, data:String }
#[derive(Default)]
struct PickerSnapshot(Mutex<Option<ScreenShot>>);
#[tauri::command]
fn screen_snapshot(state:State<PickerSnapshot>)->Result<ScreenShot,String> {
 state.0.lock().unwrap().clone().ok_or("未准备截图".into())
}
fn capture_snapshot()->ScreenShot {
 let w=unsafe{GetSystemMetrics(SYSTEM_METRICS_INDEX(0))}; let h=unsafe{GetSystemMetrics(SYSTEM_METRICS_INDEX(1))}; let raw=capture(0,0,w,h); let stride=((w*3+3)/4)*4; let file_size=54+(stride*h) as usize; let mut bmp=vec![0u8;file_size]; bmp[0]=0x42; bmp[1]=0x4d; bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes()); bmp[10..14].copy_from_slice(&(54u32).to_le_bytes()); bmp[14..18].copy_from_slice(&(40u32).to_le_bytes()); bmp[18..22].copy_from_slice(&(w as i32).to_le_bytes()); bmp[22..26].copy_from_slice(&(-(h as i32)).to_le_bytes()); bmp[26..28].copy_from_slice(&(1u16).to_le_bytes()); bmp[28..30].copy_from_slice(&(24u16).to_le_bytes()); bmp[34..38].copy_from_slice(&((stride*h) as u32).to_le_bytes()); bmp[54..].copy_from_slice(&raw); ScreenShot{x:0,y:0,width:w,height:h,stride:0,data:base64::engine::general_purpose::STANDARD.encode(bmp)}
}
#[tauri::command] async fn open_picker(app:AppHandle)->Result<(),String>{
 tauri::async_runtime::spawn_blocking(move || {
 if let Some(picker)=app.get_webview_window("picker") { picker.close().map_err(|e|e.to_string())?; }
 *app.state::<PickerSnapshot>().0.lock().unwrap()=Some(capture_snapshot());
 WebviewWindowBuilder::new(&app,"picker",WebviewUrl::App("picker.html".into())).title("拖动选择区域").decorations(false).always_on_top(true).fullscreen(true).build().map(|_|()).map_err(|e|e.to_string())
 }).await.map_err(|e|e.to_string())?
}
#[tauri::command] fn finish_picker(app:AppHandle,x:i32,y:i32,w:i32,h:i32)->Result<(),String>{ app.emit("region-selected",serde_json::json!({"x":x,"y":y,"w":w,"h":h})).map_err(|e|e.to_string())?; if let Some(picker)=app.get_webview_window("picker"){ picker.close().map_err(|e|e.to_string())?; } Ok(()) }
#[tauri::command] fn cancel_picker(app:AppHandle)->Result<(),String>{ if let Some(picker)=app.get_webview_window("picker"){ picker.close().map_err(|e|e.to_string())?; } Ok(()) }
#[tauri::command] async fn stop_monitor(app:AppHandle)->Result<(),String>{
 tauri::async_runtime::spawn_blocking(move || {
 let state=app.state::<MonitorState>();
 let mut slot=state.0.lock().unwrap();
 if let Some((handle,stop))=slot.take(){
  stop.store(true,Ordering::Relaxed);
  handle.thread().unpark();
  handle.join().map_err(|_|"检测线程异常退出".to_string())?;
 }
 Ok(())
 }).await.map_err(|e|e.to_string())?
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(){ tauri::Builder::default().manage(notifications::Notifications::default()).manage(PickerSnapshot::default()).manage(MonitorState(Arc::new(Mutex::new(None)))).invoke_handler(tauri::generate_handler![start_monitor,stop_monitor,open_picker,screen_snapshot,finish_picker,cancel_picker,test_notification]).run(tauri::generate_context!()).expect("error while running tauri application"); }

#[cfg(test)]
mod tests {
 use super::validate_threshold;
 #[test]
 fn threshold_rejects_values_that_cannot_detect_changes() {
  for value in [0.0, 75.0, 254.0] { assert!(validate_threshold(value).is_ok()); }
  for value in [-1.0, 255.0, 765.0, f32::NAN, f32::INFINITY] { assert!(validate_threshold(value).is_err()); }
 }
}













