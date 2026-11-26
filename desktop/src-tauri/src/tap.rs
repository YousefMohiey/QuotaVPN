//! The Spaces layer-2 adapter.
//!
//! TAP-Windows6 presents a virtual Ethernet card and hands raw 802.3 frames to
//! whoever opens `\\.\Global\{guid}.tap`. The driver adds the Ethernet header
//! and nothing else, so broadcast, multicast, ARP and IPv6 discovery work.
//!
//! The driver is installed by the QuotaVPN installer and appears as
//! `TAP-Windows Adapter V9`. This module adopts that one adapter, gives it the
//! member's address and MTU, brings the media up, and leaves it up until the
//! engine stops. It never touches a TAP adapter that another product installed:
//! the GUID is recorded at install time.
//!
//! The handle is synchronous. One thread blocks in `ReadFile` and feeds a
//! channel, writes are plain `WriteFile` calls, and `CancelSynchronousIo` lets
//! the reader return when the engine stops. The overlapped form read frames
//! correctly but frames written that way never reached the Windows stack.

use quotacards::spaces_forward::PacketIo;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_SYSTEM: u32 = 4;
const TAP_IOCTL_GET_MTU: u32 = 0x0022_000C;
const TAP_IOCTL_SET_MEDIA_STATUS: u32 = 0x0022_0018;
const THREAD_ALL_ACCESS: u32 = 0x001F_03FF;

/// One frame, including the Ethernet header.
pub const FRAME: usize = 2048;

#[allow(non_snake_case)]
extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        disposition: u32,
        flags: u32,
        template: *mut c_void,
    ) -> isize;
    fn DeviceIoControl(
        handle: isize,
        code: u32,
        input: *const u8,
        input_size: u32,
        output: *mut u8,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn ReadFile(
        handle: isize,
        buffer: *mut u8,
        length: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        handle: isize,
        buffer: *const u8,
        length: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn GetLastError() -> u32;
    fn GetCurrentThreadId() -> u32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> isize;
    fn CancelSynchronousIo(thread: isize) -> i32;
}

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Where the installer records the adapter it created.
pub fn marker_path() -> PathBuf {
    quotacards::config::AppConfig::config_dir().join("spaces_tap.txt")
}

pub struct TapIo {
    handle: isize,
    name: String,
    mac: [u8; 6],
    frames: Receiver<Vec<u8>>,
    reader: Option<JoinHandle<()>>,
    reader_thread: Arc<Mutex<isize>>,
}

unsafe impl Send for TapIo {}

impl TapIo {
    pub fn new(address: &str, mtu: u32) -> Result<Self, String> {
        let guid = resolve_guid()?;
        Self::adopt(&guid, address, mtu)
    }

    pub fn adopt(guid: &str, address: &str, mtu: u32) -> Result<Self, String> {
        let (name, ifindex, mac) = configure(guid, address, mtu)?;
        let path = format!("\\\\.\\Global\\{{{guid}}}.tap");
        let handle = open(&path)?;
        if let Err(error) = set_media(handle, true) {
            unsafe { CloseHandle(handle) };
            return Err(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
        set_private(ifindex);

        let (sender, frames) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = stop.clone();
        let reader_thread = Arc::new(Mutex::new(0isize));
        let thread_slot = reader_thread.clone();
        let reader = std::thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let thread_handle = unsafe { OpenThread(THREAD_ALL_ACCESS, 0, thread_id) };
            *thread_slot.lock().unwrap_or_else(|e| e.into_inner()) = thread_handle;
            let mut buffer = vec![0u8; FRAME];
            while !reader_stop.load(Ordering::Relaxed) {
                let mut read = 0u32;
                let ok = unsafe {
                    ReadFile(
                        handle,
                        buffer.as_mut_ptr(),
                        FRAME as u32,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    break;
                }
                if read as usize >= 14
                    && sender.send(buffer[..read as usize].to_vec()).is_err()
                {
                    break;
                }
            }
        });

        let io = Self {
            handle,
            name,
            mac,
            frames,
            reader: Some(reader),
            reader_thread,
        };
        quotacards::vpn::app_log(&format!(
            "spaces l2: adapter {} up, mac {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}, mtu {}",
            io.name, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], mtu
        ));
        Ok(io)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }
}

impl PacketIo for TapIo {
    fn try_read(&mut self) -> Result<Option<Vec<u8>>, String> {
        match self.frames.try_recv() {
            Ok(frame) => Ok(Some(frame)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("the Spaces adapter reader stopped".into()),
        }
    }

    fn write(&mut self, frame: &[u8]) -> Result<(), String> {
        if frame.is_empty() {
            return Ok(());
        }
        if frame.len() > FRAME {
            return Err("a frame is larger than the adapter accepts".into());
        }
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                self.handle,
                frame.as_ptr(),
                frame.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "the Spaces adapter write failed ({})",
                unsafe { GetLastError() }
            ));
        }
        Ok(())
    }
}

impl Drop for TapIo {
    fn drop(&mut self) {
        let thread_handle = *self
            .reader_thread
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if thread_handle != 0 {
            unsafe {
                CancelSynchronousIo(thread_handle);
            }
        }
        let _ = set_media(self.handle, false);
        unsafe {
            CloseHandle(self.handle);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if thread_handle != 0 {
            unsafe {
                CloseHandle(thread_handle);
            }
        }
    }
}

fn open(path: &str) -> Result<isize, String> {
    let name = wide(path);
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_SYSTEM,
            std::ptr::null_mut(),
        )
    };
    if handle == -1 || handle == 0 {
        return Err(format!(
            "the Spaces adapter device could not be opened ({})",
            unsafe { GetLastError() }
        ));
    }
    let mut output = [0u8; 16];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            TAP_IOCTL_GET_MTU,
            std::ptr::null(),
            0,
            output.as_mut_ptr(),
            output.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        let error = unsafe { GetLastError() };
        unsafe {
            CloseHandle(handle);
        }
        return Err(format!(
            "the Spaces adapter is not a TAP-Windows6 device ({error})"
        ));
    }
    Ok(handle)
}

fn set_media(handle: isize, up: bool) -> Result<(), String> {
    let mut status: u32 = if up { 1 } else { 0 };
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            TAP_IOCTL_SET_MEDIA_STATUS,
            &mut status as *mut u32 as *const u8,
            4,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "the Spaces adapter did not change media state ({})",
            unsafe { GetLastError() }
        ));
    }
    Ok(())
}

fn resolve_guid() -> Result<String, String> {
    if let Ok(saved) = std::fs::read_to_string(marker_path()) {
        let saved = saved.trim().trim_matches('{').trim_matches('}').to_string();
        if !saved.is_empty() && adapter_exists(&saved) {
            return Ok(saved);
        }
    }
    let found = first_tap_adapter()?;
    if let Some(dir) = marker_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(marker_path(), &found);
    Ok(found)
}

fn adapter_exists(guid: &str) -> bool {
    run_powershell(&format!(
        "$g = '{{{guid}}}'; $a = Get-NetAdapter | Where-Object {{ \"$($_.InterfaceGuid)\" -eq $g }}; if ($a) {{ 'yes' }} else {{ 'no' }}"
    ))
    .map(|out| out.trim() == "yes")
    .unwrap_or(false)
}

fn first_tap_adapter() -> Result<String, String> {
    let out = run_powershell(
        "$all = @(Get-NetAdapter | Where-Object { $_.InterfaceDescription -like '*TAP-Windows*' }); \
         $ours = $all | Where-Object { $_.Name -like 'QuotaSpace*' } | Sort-Object ifIndex | Select-Object -First 1; \
         if ($ours) { $ours.InterfaceGuid } elseif ($all.Count -eq 1) { $all[0].InterfaceGuid } else { '' }",
    )?;
    let guid = out.trim().trim_matches('{').trim_matches('}').to_string();
    if guid.is_empty() {
        return Err(
            "the Spaces layer-2 adapter is not installed; run the QuotaVPN installer again".into(),
        );
    }
    Ok(guid)
}

/// The MAC is deliberately not overridden: frames addressed to a NetworkAddress
/// override were not accepted on receive in testing. The driver MAC is used.
fn configure(guid: &str, address: &str, mtu: u32) -> Result<(String, i32, [u8; 6]), String> {
    let ip = address.split('/').next().unwrap_or(address).trim();
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $g = '{{{guid}}}'; \
         $a = Get-NetAdapter | Where-Object {{ \"$($_.InterfaceGuid)\" -eq $g }} | Select-Object -First 1; \
         if (-not $a) {{ throw 'the Spaces adapter was not found' }}; \
         $name = $a.Name; \
         if ($name -notlike 'QuotaSpace*') {{ \
           $taken = Get-NetAdapter -Name 'QuotaSpaceLAN' -ErrorAction SilentlyContinue; \
           if ($taken) {{ throw 'another adapter already uses QuotaSpaceLAN' }}; \
           Rename-NetAdapter -Name $name -NewName 'QuotaSpaceLAN'; $name = 'QuotaSpaceLAN' }}; \
         Set-NetAdapterAdvancedProperty -Name $name -RegistryKeyword MTU -RegistryValue {mtu} -ErrorAction SilentlyContinue | Out-Null; \
         netsh interface ipv4 set address name=\"$name\" source=static address={ip} mask=255.255.255.0 gateway=none | Out-Null; \
         $a = Get-NetAdapter -Name $name; \
         \"$($a.Name)|$($a.ifIndex)|$($a.MacAddress)\""
    );
    let out = run_powershell(&script)?;
    let mut parts = out.trim().split('|');
    let name = parts.next().unwrap_or_default().to_string();
    let ifindex = parts
        .next()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .ok_or("the Spaces adapter did not report an interface index")?;
    let mac_text = parts.next().unwrap_or_default().trim();
    if name.is_empty() {
        return Err("the Spaces adapter did not report a name".into());
    }
    let mac = parse_mac(mac_text).ok_or("the Spaces adapter did not report a MAC")?;
    Ok((name, ifindex, mac))
}

fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let clean: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 12 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (index, slot) in mac.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&clean[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(mac)
}

fn set_private(ifindex: i32) {
    let _ = run_powershell(&format!(
        "Set-NetConnectionProfile -InterfaceIndex {ifindex} -NetworkCategory Private -ErrorAction SilentlyContinue"
    ));
}

fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(format!("the Spaces adapter command failed: {detail}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn the_adapter_comes_up_and_carries_a_frame() {
        let mut io = TapIo::new("10.77.99.2/24", 1380).expect("the Spaces adapter");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        let mut frames = 0;
        while std::time::Instant::now() < deadline {
            match io.try_read() {
                Ok(Some(frame)) => {
                    if frame.len() >= 14 {
                        frames += 1;
                        break;
                    }
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(error) => panic!("read: {error}"),
            }
        }
        assert!(frames > 0, "no frame arrived from the adapter");
    }
}
