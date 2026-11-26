//! Minimal Wintun adapter for the Spaces direct path.
//!
//! The QuotaVPN engine already installs `wintun.dll`; this loads that DLL by
//! path and creates a second adapter, so Spaces does not need a TAP driver or a
//! second VPN product. The adapter is L3, which is enough for unicast IP and for
//! forwarding IPv4 broadcasts to the peer.
//!
//! Only Windows is compiled. The handle is moved to the single forwarding
//! thread and never shared, so the raw pointers have a `Send` bound, not `Sync`.

use libloading::Library;
use quotacards::spaces_forward::PacketIo;
use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ERROR_NO_MORE_ITEMS: i32 = 259;
const RING_CAPACITY: u32 = 0x40_0000;

type CreateAdapter =
    unsafe extern "system" fn(*const u16, *const u16, *const c_void) -> *mut c_void;
type OpenAdapter = unsafe extern "system" fn(*const u16) -> *mut c_void;
type CloseAdapter = unsafe extern "system" fn(*mut c_void);
type StartSession = unsafe extern "system" fn(*mut c_void, u32) -> *mut c_void;
type EndSession = unsafe extern "system" fn(*mut c_void);
type ReceivePacket = unsafe extern "system" fn(*mut c_void, *mut u32) -> *mut u8;
type ReleasePacket = unsafe extern "system" fn(*mut c_void, *const u8);
type AllocatePacket = unsafe extern "system" fn(*mut c_void, u32) -> *mut u8;
type SendPacket = unsafe extern "system" fn(*mut c_void, *const u8);

pub struct WintunIo {
    _lib: Library,
    adapter: *mut c_void,
    session: *mut c_void,
    name: String,
    close_adapter: CloseAdapter,
    end_session: EndSession,
    receive_packet: ReceivePacket,
    release_packet: ReleasePacket,
    allocate_packet: AllocatePacket,
    send_packet: SendPacket,
}

unsafe impl Send for WintunIo {}

impl WintunIo {
    /// Create or reopen the adapter and give it the member's `10.77.n.x/24`.
    pub fn new(space_id: &str, address: &str) -> Result<Self, String> {
        let ip: Ipv4Addr = address
            .split('/')
            .next()
            .unwrap_or(address)
            .trim()
            .parse()
            .map_err(|_| "the space address is not IPv4".to_string())?;
        let suffix: String = space_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(4)
            .collect();
        let name = format!("QuotaSpace{}", if suffix.is_empty() { "0000" } else { &suffix });
        let path = quotacards::vpn::engine_dir().join("wintun.dll");
        if !path.exists() {
            quotacards::vpn::ensure_wintun()?;
        }
        let lib = unsafe { Library::new(&path) }
            .or_else(|_| unsafe { Library::new("wintun.dll") })
            .map_err(|e| format!("the Wintun driver could not be loaded ({e})"))?;
        let create: CreateAdapter = unsafe {
            *lib.get(b"WintunCreateAdapter\0")
                .map_err(|_| "wintun.dll is missing WintunCreateAdapter".to_string())?
        };
        let open: OpenAdapter = unsafe {
            *lib.get(b"WintunOpenAdapter\0")
                .map_err(|_| "wintun.dll is missing WintunOpenAdapter".to_string())?
        };
        let close_adapter: CloseAdapter = unsafe {
            *lib.get(b"WintunCloseAdapter\0")
                .map_err(|_| "wintun.dll is missing WintunCloseAdapter".to_string())?
        };
        let start_session: StartSession = unsafe {
            *lib.get(b"WintunStartSession\0")
                .map_err(|_| "wintun.dll is missing WintunStartSession".to_string())?
        };
        let end_session: EndSession = unsafe {
            *lib.get(b"WintunEndSession\0")
                .map_err(|_| "wintun.dll is missing WintunEndSession".to_string())?
        };
        let receive_packet: ReceivePacket = unsafe {
            *lib.get(b"WintunReceivePacket\0")
                .map_err(|_| "wintun.dll is missing WintunReceivePacket".to_string())?
        };
        let release_packet: ReleasePacket = unsafe {
            *lib.get(b"WintunReleaseReceivePacket\0")
                .map_err(|_| "wintun.dll is missing WintunReleaseReceivePacket".to_string())?
        };
        let allocate_packet: AllocatePacket = unsafe {
            *lib.get(b"WintunAllocateSendPacket\0")
                .map_err(|_| "wintun.dll is missing WintunAllocateSendPacket".to_string())?
        };
        let send_packet: SendPacket = unsafe {
            *lib.get(b"WintunSendPacket\0")
                .map_err(|_| "wintun.dll is missing WintunSendPacket".to_string())?
        };

        let wide_name = wide(&name);
        let tunnel = wide("QuotaVPN");
        let mut adapter = unsafe {
            create(
                wide_name.as_ptr(),
                tunnel.as_ptr(),
                std::ptr::null(),
            )
        };
        if adapter.is_null() {
            // A force-killed session leaves the adapter behind. Reuse it
            // instead of asking the user to reboot.
            adapter = unsafe { open(wide_name.as_ptr()) };
        }
        if adapter.is_null() {
            return Err("the Spaces adapter could not be created".into());
        }
        let session = unsafe { start_session(adapter, RING_CAPACITY) };
        if session.is_null() {
            unsafe { close_adapter(adapter) };
            return Err("the Spaces adapter session could not start".into());
        }
        let io = Self {
            _lib: lib,
            adapter,
            session,
            name,
            close_adapter,
            end_session,
            receive_packet,
            release_packet,
            allocate_packet,
            send_packet,
        };
        if let Err(error) = io.configure(ip) {
            drop(io);
            return Err(error);
        }
        Ok(io)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn configure(&self, ip: Ipv4Addr) -> Result<(), String> {
        let address = ip.to_string();
        let mut last = String::new();
        for _ in 0..20 {
            match netsh(&[
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name={}", self.name),
                "static",
                &address,
                "255.255.255.0",
            ]) {
                Ok(()) => {
                    let _ = netsh(&[
                        "interface",
                        "ipv4",
                        "set",
                        "interface",
                        &self.name,
                        "metric=1",
                    ]);
                    let _ = netsh(&[
                        "interface",
                        "ipv4",
                        "set",
                        "subinterface",
                        &self.name,
                        "mtu=1280",
                        "store=persistent",
                    ]);
                    // A new adapter defaults to the Public profile, where the
                    // game's inbound firewall rule usually does not apply.
                    // Private is the LAN profile the space is meant to be.
                    let _ = Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-WindowStyle",
                            "Hidden",
                            "-Command",
                            &format!(
                                "Set-NetConnectionProfile -InterfaceAlias '{}' -NetworkCategory Private -ErrorAction SilentlyContinue",
                                self.name
                            ),
                        ])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output();
                    return Ok(());
                }
                Err(error) => last = error,
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err(format!(
            "the Spaces adapter did not accept its address: {last}"
        ))
    }
}

impl PacketIo for WintunIo {
    fn try_read(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut size = 0u32;
        let packet = unsafe { (self.receive_packet)(self.session, &mut size) };
        if packet.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS) {
                return Ok(None);
            }
            return Err(format!("Spaces adapter receive: {error}"));
        }
        let bytes = if size == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(packet, size as usize) }.to_vec()
        };
        unsafe { (self.release_packet)(self.session, packet) };
        Ok(Some(bytes))
    }

    fn write(&mut self, packet: &[u8]) -> Result<(), String> {
        if packet.is_empty() || packet.len() > u32::MAX as usize {
            return Ok(());
        }
        let target = unsafe { (self.allocate_packet)(self.session, packet.len() as u32) };
        if target.is_null() {
            return Err("the Spaces adapter send ring is full".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(packet.as_ptr(), target, packet.len());
            (self.send_packet)(self.session, target);
        }
        Ok(())
    }
}

impl Drop for WintunIo {
    fn drop(&mut self) {
        unsafe {
            if !self.session.is_null() {
                (self.end_session)(self.session);
                self.session = std::ptr::null_mut();
            }
            if !self.adapter.is_null() {
                (self.close_adapter)(self.adapter);
                self.adapter = std::ptr::null_mut();
            }
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn netsh(args: &[&str]) -> Result<(), String> {
    let output = Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("netsh: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr);
    let text = text.trim();
    Err(if text.is_empty() {
        "netsh refused the adapter change".into()
    } else {
        text.chars().take(160).collect()
    })
}
