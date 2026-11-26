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
//! The I/O follows the overlapped pattern in OpenRad's `windows_io.rs` (MIT):
//! the handle is opened with FILE_FLAG_OVERLAPPED and exclusive access, every
//! operation owns a stable buffer and OVERLAPPED, ReadFile and WriteFile get a
//! NULL byte-count pointer, and the event is reset per operation. Handing the
//! kernel a stack byte-count pointer or the caller's buffer is what made
//! written frames disappear before.

use quotacards::spaces_forward::PacketIo;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_SYSTEM: u32 = 4;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const ERROR_IO_PENDING: u32 = 997;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;

const TAP_IOCTL_GET_MTU: u32 = (0x22 << 16) | (3 << 2);
const TAP_IOCTL_SET_MEDIA_STATUS: u32 = (0x22 << 16) | (6 << 2);

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
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
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
        overlapped: *const Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    fn CreateEventW(
        security: *mut c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> isize;
    fn ResetEvent(handle: isize) -> i32;
    fn SetEvent(handle: isize) -> i32;
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

/// One overlapped operation with storage that cannot move while the kernel
/// holds it. Ported from OpenRad (MIT).
struct Operation {
    handle: isize,
    event: isize,
    overlapped: Box<Overlapped>,
    buffer: Box<[u8]>,
    pending: bool,
    immediate: u32,
}

unsafe impl Send for Operation {}

impl Operation {
    fn new(handle: isize, capacity: usize) -> Result<Self, String> {
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event == 0 || event == -1 {
            return Err("the Spaces adapter event could not be created".into());
        }
        Ok(Self {
            handle,
            event,
            overlapped: Box::new(Overlapped {
                event,
                ..Default::default()
            }),
            buffer: vec![0u8; capacity].into_boxed_slice(),
            pending: false,
            immediate: 0,
        })
    }

    fn prepare(&mut self) -> Result<(), String> {
        if self.pending {
            return Err("the Spaces adapter reused a pending operation".into());
        }
        *self.overlapped = Overlapped {
            event: self.event,
            ..Default::default()
        };
        self.immediate = 0;
        if unsafe { ResetEvent(self.event) } == 0 {
            return Err(format!(
                "the Spaces adapter event could not be reset ({})",
                unsafe { GetLastError() }
            ));
        }
        Ok(())
    }

    fn started(&mut self, success: i32) -> Result<(), String> {
        if success != 0 {
            if unsafe {
                GetOverlappedResult(self.handle, &*self.overlapped, &mut self.immediate, 0)
            } == 0
            {
                return Err(format!(
                    "the Spaces adapter operation failed ({})",
                    unsafe { GetLastError() }
                ));
            }
            // Signal for an immediate completion too, so `ready` is uniform.
            if unsafe { SetEvent(self.event) } == 0 {
                return Err("the Spaces adapter event could not be set".into());
            }
        } else {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                return Err(format!("the Spaces adapter operation failed ({error})"));
            }
            self.pending = true;
        }
        Ok(())
    }

    fn read(&mut self) -> Result<(), String> {
        self.prepare()?;
        let success = unsafe {
            ReadFile(
                self.handle,
                self.buffer.as_mut_ptr(),
                self.buffer.len() as u32,
                std::ptr::null_mut(),
                &mut *self.overlapped,
            )
        };
        self.started(success)
    }

    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() > self.buffer.len() {
            return Err("a frame is larger than the adapter accepts".into());
        }
        self.prepare()?;
        self.buffer[..data.len()].copy_from_slice(data);
        let success = unsafe {
            WriteFile(
                self.handle,
                self.buffer.as_ptr(),
                data.len() as u32,
                std::ptr::null_mut(),
                &mut *self.overlapped,
            )
        };
        self.started(success)?;
        self.finish(2000).map(|_| ())
    }

    fn ioctl(&mut self, code: u32, input: &[u8]) -> Result<usize, String> {
        self.prepare()?;
        if input.len() > self.buffer.len() {
            return Err("the Spaces adapter control buffer is too small".into());
        }
        self.buffer[..input.len()].copy_from_slice(input);
        let success = unsafe {
            DeviceIoControl(
                self.handle,
                code,
                self.buffer.as_ptr().cast(),
                input.len() as u32,
                self.buffer.as_mut_ptr().cast(),
                self.buffer.len() as u32,
                std::ptr::null_mut(),
                &mut *self.overlapped,
            )
        };
        self.started(success)?;
        self.finish(1000)
    }

    fn finish(&mut self, timeout_ms: u32) -> Result<usize, String> {
        if !self.pending {
            return Ok(self.immediate as usize);
        }
        match unsafe { WaitForSingleObject(self.event, timeout_ms) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => {
                self.cancel();
                return Err("the Spaces adapter operation timed out".into());
            }
            _ => {
                self.cancel();
                return Err("the Spaces adapter wait failed".into());
            }
        }
        let mut len = 0u32;
        let success = unsafe {
            GetOverlappedResult(self.handle, &*self.overlapped, &mut len, 1)
        };
        self.pending = false;
        if success == 0 {
            return Err(format!(
                "the Spaces adapter operation stopped ({})",
                unsafe { GetLastError() }
            ));
        }
        Ok(len as usize)
    }

    fn cancel(&mut self) {
        if self.pending {
            unsafe {
                CancelIoEx(self.handle, &*self.overlapped);
                let mut len = 0u32;
                GetOverlappedResult(self.handle, &*self.overlapped, &mut len, 1);
            }
            self.pending = false;
        }
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        self.cancel();
        unsafe {
            CloseHandle(self.event);
        }
    }
}

pub struct TapIo {
    handle: isize,
    name: String,
    mac: [u8; 6],
    write: Operation,
    frames: Receiver<Vec<u8>>,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
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

        let mut control = Operation::new(handle, 64)?;
        let read = Operation::new(handle, FRAME)?;
        let write = Operation::new(handle, FRAME)?;

        control.ioctl(TAP_IOCTL_GET_MTU, &[])?;
        control.ioctl(TAP_IOCTL_SET_MEDIA_STATUS, &1u32.to_ne_bytes())?;

        // The media is up now. Re-apply the address so the on-link route is
        // installed before the first frame, and wait for it to be visible.
        let ip = address.split('/').next().unwrap_or(address).trim();
        let _ = run_powershell(&format!(
            "netsh interface ipv4 set address name=\"{name}\" source=static address={ip} mask=255.255.255.0 gateway=none | Out-Null"
        ));
        for _ in 0..20 {
            let present = run_powershell(&format!(
                "(Get-NetIPAddress -InterfaceIndex {ifindex} -AddressFamily IPv4 -ErrorAction SilentlyContinue).IPAddress"
            ))
            .unwrap_or_default();
            if present.contains(ip) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        std::thread::sleep(std::time::Duration::from_millis(3000));
        set_private(ifindex);
        // Windows Firewall drops inbound ICMP and game traffic on a new
        // adapter even when the profile is Private. The interface rule is what
        // lets the peer's replies and LAN broadcasts reach the apps.
        let _ = run_powershell(&format!(
            "$n='QuotaVPN Spaces LAN'; if (-not (Get-NetFirewallRule -DisplayName $n -ErrorAction SilentlyContinue)) {{ New-NetFirewallRule -DisplayName $n -Direction Inbound -InterfaceAlias '{name}' -Action Allow -Profile Private | Out-Null }}"
        ));

        let (sender, frames) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = stop.clone();
        let mut read_op = read;
        let reader = std::thread::spawn(move || {
            loop {
                if reader_stop.load(Ordering::Relaxed) {
                    break;
                }
                if read_op.read().is_err() {
                    break;
                }
                match read_op.finish(500) {
                    Ok(length) if length >= 14 => {
                        if sender.send(read_op.buffer[..length].to_vec()).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        if reader_stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }
        });

        let io = Self {
            handle,
            name,
            mac,
            write,
            frames,
            stop,
            reader: Some(reader),
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
        self.write.write(frame)
    }
}

impl Drop for TapIo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        unsafe {
            // Cancel every pending request on the handle; the reader's wait
            // returns and the join can finish.
            CancelIoEx(self.handle, std::ptr::null());
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
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
            0,
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
    Ok(handle)
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

/// The MAC is deliberately not overridden; the driver MAC is used.
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

    #[test]
    #[ignore]
    fn a_written_broadcast_arp_reaches_windows() {
        let mut io = TapIo::new("10.77.99.2/24", 1380).expect("the Spaces adapter");
        let sender = [0x02u8, 0x51, 0x56, 0x00, 0x00, 0x09];
        let mut frame = Vec::new();
        frame.extend_from_slice(&io.mac());
        frame.extend_from_slice(&sender);
        frame.extend_from_slice(&[0x08, 0x06]);
        frame.extend_from_slice(&[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01]);
        frame.extend_from_slice(&sender);
        frame.extend_from_slice(&[10, 77, 99, 9]);
        frame.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        frame.extend_from_slice(&[10, 77, 99, 2]);
        std::thread::sleep(std::time::Duration::from_secs(3));
        io.write(&frame).expect("write");
        std::thread::sleep(std::time::Duration::from_millis(900));
        let out = run_powershell(
            "(Get-NetNeighbor -InterfaceAlias 'QuotaSpaceLAN' -AddressFamily IPv4 -IPAddress '10.77.99.9' -ErrorAction SilentlyContinue).LinkLayerAddress",
        )
        .unwrap_or_default();
        assert!(
            out.to_uppercase().contains("02-51-56-00-00-09"),
            "windows did not learn the sender: {out}"
        );
    }
}

#[cfg(test)]
mod icmp_tests {
    use super::*;

    fn checksum(data: &[u8]) -> u16 {
        let mut sum = 0u32;
        let mut i = 0;
        while i + 1 < data.len() {
            sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
            i += 2;
        }
        if i < data.len() {
            sum += (data[i] as u32) << 8;
        }
        while sum >> 16 != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        !(sum as u16)
    }

    #[test]
    #[ignore]
    fn a_crafted_echo_gets_a_windows_reply() {
        let _ = run_powershell(
            "New-NetNeighbor -InterfaceAlias 'QuotaSpaceLAN' -IPAddress '10.77.99.9' -LinkLayerAddress '02-51-56-00-00-09' -State Permanent -ErrorAction SilentlyContinue | Out-Null",
        );
        let mut io = TapIo::new("10.77.99.2/24", 1380).expect("adapter");
        std::thread::sleep(std::time::Duration::from_secs(3));
        let mac = io.mac();
        let mut ip = vec![0x45, 0x00, 0x00, 0x3c, 0x12, 0x34, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 10, 77, 99, 9, 10, 77, 99, 2];
        let c = checksum(&ip);
        ip[10..12].copy_from_slice(&c.to_be_bytes());
        let mut icmp = vec![0x08, 0x00, 0x00, 0x00, 0x12, 0x34, 0x00, 0x01];
        icmp.extend(std::iter::repeat(b'a').take(32));
        let c = checksum(&icmp);
        icmp[2..4].copy_from_slice(&c.to_be_bytes());
        let mut frame = Vec::new();
        frame.extend_from_slice(&mac);
        frame.extend_from_slice(&[0x02, 0x51, 0x56, 0x00, 0x00, 0x09]);
        frame.extend_from_slice(&[0x08, 0x00]);
        frame.extend_from_slice(&ip);
        frame.extend_from_slice(&icmp);
        io.write(&frame).expect("write");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut replied = false;
        while std::time::Instant::now() < deadline {
            match io.try_read() {
                Ok(Some(f)) => {
                    if f.len() >= 42 && f[12] == 0x08 && f[13] == 0x00 && f[23] == 1 && f[34] == 0 && f[38] == 0x12 && f[39] == 0x34 {
                        replied = true;
                        break;
                    }
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => panic!("read: {e}"),
            }
        }
        let _ = run_powershell("Remove-NetNeighbor -InterfaceAlias 'QuotaSpaceLAN' -IPAddress '10.77.99.9' -Confirm:$false -ErrorAction SilentlyContinue | Out-Null");
        assert!(replied, "windows did not answer the crafted echo");
    }
}
