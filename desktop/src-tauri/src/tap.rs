//! The Spaces layer-2 adapter.
//!
//! TAP-Windows6 presents a virtual Ethernet card and hands raw 802.3 frames to
//! whoever opens `\\.\Global\{guid}.tap`. The driver adds the Ethernet header
//! and nothing else, so broadcast, multicast, ARP and IPv6 discovery all work
//! the way they do on a real LAN.
//!
//! The driver is installed by the QuotaVPN installer and appears as
//! `TAP-Windows Adapter V9`. This module adopts that one adapter, gives it the
//! member's MAC, address and MTU, brings the media up, and leaves it up until
//! the engine stops. It never touches a TAP adapter that another product
//! installed: the GUID is recorded at install time.

use quotacards::spaces_forward::PacketIo;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_SYSTEM: u32 = 4;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const ERROR_IO_PENDING: u32 = 997;
const INFINITE: u32 = 0xFFFF_FFFF;

const TAP_IOCTL_GET_MTU: u32 = 0x0022_000C;
const TAP_IOCTL_SET_MEDIA_STATUS: u32 = 0x0022_0018;

/// One frame, including the Ethernet header.
pub const FRAME: usize = 2048;

#[repr(C)]
#[derive(Default)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: isize,
}

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
        overlapped: *mut Overlapped,
    ) -> i32;
    fn ReadFile(
        handle: isize,
        buffer: *mut u8,
        length: u32,
        read: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn WriteFile(
        handle: isize,
        buffer: *const u8,
        length: u32,
        written: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn GetOverlappedResult(
        handle: isize,
        overlapped: *mut Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    fn CreateEventW(
        security: *mut c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> isize;
    fn WaitForSingleObject(handle: isize, milliseconds: u32) -> u32;
    fn CancelIoEx(handle: isize, overlapped: *const Overlapped) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn GetLastError() -> u32;
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
    pending: Option<PendingRead>,
}

struct PendingRead {
    buffer: Box<[u8; FRAME]>,
    overlapped: Box<Overlapped>,
    event: isize,
    ready: Option<usize>,
}

unsafe impl Send for TapIo {}

impl TapIo {
    /// Adopt the recorded adapter, configure it, and bring the media up.
    pub fn new(mac: [u8; 6], address: &str, mtu: u32) -> Result<Self, String> {
        let guid = resolve_guid()?;
        Self::adopt(&guid, mac, address, mtu)
    }

    /// Same, for an adapter named by GUID. The engine uses `new`; the two
    /// adapter test uses this directly.
    pub fn adopt(guid: &str, mac: [u8; 6], address: &str, mtu: u32) -> Result<Self, String> {
        let (name, ifindex) = configure(guid, mac, address, mtu)?;
        let path = format!("\\\\.\\Global\\{{{guid}}}.tap");
        let handle = open(&path)?;
        if let Err(error) = set_media(handle, true) {
            unsafe { CloseHandle(handle) };
            return Err(error);
        }
        let io = Self {
            handle,
            name,
            pending: None,
        };
        // The adapter only gets a network profile once the media is up.
        std::thread::sleep(std::time::Duration::from_millis(600));
        set_private(ifindex);
        quotacards::vpn::app_log(&format!(
            "spaces l2: adapter {} up, mac {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}, mtu {}",
            io.name, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], mtu
        ));
        Ok(io)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn start_read(&mut self) -> Result<(), String> {
        let mut buffer = Box::new([0u8; FRAME]);
        let mut overlapped = Box::new(Overlapped::default());
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event == 0 || event == -1 {
            return Err("the Spaces adapter read event could not be created".into());
        }
        overlapped.event = event;
        let mut read = 0u32;
        let ok = unsafe {
            ReadFile(
                self.handle,
                buffer.as_mut_ptr(),
                FRAME as u32,
                &mut read,
                &mut *overlapped,
            )
        };
        let ready = if ok != 0 {
            Some(read as usize)
        } else {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                unsafe {
                    CloseHandle(event);
                }
                return Err(format!("the Spaces adapter read failed ({error})"));
            }
            None
        };
        self.pending = Some(PendingRead {
            buffer,
            overlapped,
            event,
            ready,
        });
        Ok(())
    }
}

impl PacketIo for TapIo {
    fn try_read(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.pending.is_none() {
            self.start_read()?;
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(None);
        };
        if pending.ready.is_none() {
            let signalled = unsafe { WaitForSingleObject(pending.event, 0) };
            if signalled != 0 {
                return Ok(None);
            }
            let mut transferred = 0u32;
            let ok = unsafe {
                GetOverlappedResult(self.handle, &mut *pending.overlapped, &mut transferred, 0)
            };
            if ok == 0 {
                let error = unsafe { GetLastError() };
                if let Some(pending) = self.pending.take() {
                    unsafe {
                        CloseHandle(pending.event);
                    }
                }
                return Err(format!("the Spaces adapter read stopped ({error})"));
            }
            pending.ready = Some(transferred as usize);
        }
        let length = pending.ready.take().unwrap_or(0);
        let frame = pending.buffer.get(..length).unwrap_or(&[]).to_vec();
        if let Some(pending) = self.pending.take() {
            unsafe {
                CloseHandle(pending.event);
            }
        }
        Ok(Some(frame))
    }

    fn write(&mut self, frame: &[u8]) -> Result<(), String> {
        if frame.is_empty() {
            return Ok(());
        }
        if frame.len() > FRAME {
            return Err("a frame is larger than the adapter accepts".into());
        }
        let mut overlapped = Overlapped::default();
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event == 0 || event == -1 {
            return Err("the Spaces adapter write event could not be created".into());
        }
        overlapped.event = event;
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                self.handle,
                frame.as_ptr(),
                frame.len() as u32,
                &mut written,
                &mut overlapped,
            )
        };
        let result = if ok != 0 {
            Ok(())
        } else {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                Err(format!("the Spaces adapter write failed ({error})"))
            } else if unsafe { WaitForSingleObject(event, INFINITE) } != 0 {
                // Never leave an IRP pointing at this stack buffer.
                unsafe {
                    CancelIoEx(self.handle, &overlapped);
                    WaitForSingleObject(event, INFINITE);
                }
                Err("the Spaces adapter write did not complete".into())
            } else {
                let mut transferred = 0u32;
                let done = unsafe {
                    GetOverlappedResult(self.handle, &mut overlapped, &mut transferred, 0)
                };
                if done == 0 {
                    Err(format!(
                        "the Spaces adapter write stopped ({})",
                        unsafe { GetLastError() }
                    ))
                } else {
                    Ok(())
                }
            }
        };
        unsafe {
            CloseHandle(event);
        }
        result
    }
}

impl Drop for TapIo {
    fn drop(&mut self) {
        let _ = set_media(self.handle, false);
        if let Some(pending) = self.pending.take() {
            unsafe {
                CloseHandle(pending.event);
            }
        }
        unsafe {
            CloseHandle(self.handle);
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
            FILE_ATTRIBUTE_SYSTEM | FILE_FLAG_OVERLAPPED,
            std::ptr::null_mut(),
        )
    };
    if handle == -1 || handle == 0 {
        return Err(format!(
            "the Spaces adapter device could not be opened ({})",
            unsafe { GetLastError() }
        ));
    }
    // A TAP adapter answers this with its frame size. Asking for the version
    // instead returns ERROR_INSUFFICIENT_BUFFER on 9.27.0 when the output
    // buffer is only four bytes.
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

/// Up or down. Down is what tells Windows the cable is out and removes the
/// routes; Drop calls it so a crash does not leave a live adapter behind.
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

/// The adapter GUID recorded by the installer, or the only TAP adapter.
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

/// Give the adapter its MAC, MTU and address. Returns the adapter name and
/// interface index.
fn configure(guid: &str, mac: [u8; 6], address: &str, mtu: u32) -> Result<(String, i32), String> {
    let mac_hex = format!(
        "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
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
         Set-NetAdapterAdvancedProperty -Name $name -RegistryKeyword NetworkAddress -RegistryValue '{mac_hex}' -ErrorAction SilentlyContinue | Out-Null; \
         Set-NetAdapterAdvancedProperty -Name $name -RegistryKeyword MTU -RegistryValue {mtu} -ErrorAction SilentlyContinue | Out-Null; \
         netsh interface ipv4 set address name=\"$name\" source=static address={ip} mask=255.255.255.0 gateway=none | Out-Null; \
         $a = Get-NetAdapter -Name $name; \
         \"$($a.Name)|$($a.ifIndex)\""
    );
    let out = run_powershell(&script)?;
    let mut parts = out.trim().split('|');
    let name = parts.next().unwrap_or_default().to_string();
    let ifindex = parts
        .next()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .ok_or("the Spaces adapter did not report an interface index")?;
    if name.is_empty() {
        return Err("the Spaces adapter did not report a name".into());
    }
    Ok((name, ifindex))
}

fn set_private(ifindex: i32) {
    let _ = run_powershell(&format!(
        "Set-NetConnectionProfile -InterfaceIndex {ifindex} -NetworkCategory Private -ErrorAction SilentlyContinue"
    ));
}

fn run_powershell(script: &str) -> Result<String, String> {    let output = Command::new("powershell")
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

    /// Runs only on a machine with the adapter installed and an elevated shell,
    /// so it is ignored by default. It proves the Rust path that the engine
    /// uses: configure, media up, read a frame the OS sends by itself.
    #[test]
    #[ignore]
    fn the_adapter_comes_up_and_carries_a_frame() {
        let mut io = TapIo::new([0x02, 0x51, 0x56, 0x00, 0x00, 0x01], "10.77.99.2/24", 1380)
            .expect("the Spaces adapter");
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

    struct PeerAdapter {
        io: TapIo,
    }

    impl quotacards::spaces_switch::PeerLink for PeerAdapter {
        fn label(&self) -> &str {
            self.io.name()
        }

        fn send(&mut self, frame: &[u8]) -> Result<(), String> {
            self.io.write(frame)
        }

        fn try_recv(&mut self) -> Result<Option<Vec<u8>>, String> {
            self.io.try_read()
        }
    }

    fn guid_of(name: &str) -> String {
        run_powershell(&format!("(Get-NetAdapter -Name '{name}').InterfaceGuid"))
            .expect("adapter lookup")
            .trim()
            .trim_matches('{')
            .trim_matches('}')
            .to_string()
    }

    /// The end-to-end proof that does not need a second PC: two TAP adapters on
    /// this machine, in one /24, bridged by the real switch. Windows has to ARP
    /// across it and the ping has to come back.
    ///
    /// Needs `QuotaSpaceLAN` and `QuotaSpaceB` installed and an elevated shell.
    #[test]
    #[ignore]
    fn two_adapters_ping_through_the_switch() {
        use quotacards::spaces_switch::{self, PeerLink};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let a = guid_of("QuotaSpaceLAN");
        let b = guid_of("QuotaSpaceB");
        if a.is_empty() || b.is_empty() {
            panic!("install QuotaSpaceLAN and QuotaSpaceB before this test");
        }
        let mac_a = [0x02, 0x51, 0x56, 0x00, 0x01, 0x01];
        let mac_b = [0x02, 0x51, 0x56, 0x00, 0x01, 0x02];
        let mut io_a = TapIo::adopt(&a, mac_a, "10.77.98.2/24", 1380).expect("adapter A");
        let io_b = TapIo::adopt(&b, mac_b, "10.77.98.3/24", 1380).expect("adapter B");
        let mut peers: Vec<Box<dyn PeerLink>> = vec![Box::new(PeerAdapter { io: io_b })];

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_ping = stop.clone();
        let pinger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(900));
            let out = std::process::Command::new("ping")
                .args(["-n", "3", "-w", "1200", "10.77.98.3"])
                .output()
                .expect("ping");
            stop_for_ping.store(true, Ordering::Relaxed);
            out
        });

        let result = spaces_switch::run(&mut io_a, &mut peers, mac_a, &stop);
        result.expect("the switch loop");

        let out = pinger.join().expect("pinger");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "ping across the switch failed: {text}");
    }
}
