//! GPU detection and Windows graphics preferences for AMD Radeon and universal GPUs.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arch {
    /// AMD RDNA 3 (Navi 3x / RX 7000 series, e.g. RX 7700 XT) - has dedicated AI matrix cores & official INT8 support.
    Rdna3,
    /// AMD RDNA 2 (Navi 2x / RX 6000 series).
    Rdna2,
    /// AMD RDNA 1 or GCN.
    OtherAmd,
    /// Intel Arc (Alchemist / Battlemage) with DP4a / XMX.
    IntelArc,
    /// NVIDIA GeForce.
    Nvidia,
    Unknown,
}

impl Arch {
    pub fn label(&self) -> &'static str {
        match self {
            Arch::Rdna3 => "AMD RDNA 3 · Native FSR 4 INT8 & FSR 3.1 FG Accelerated",
            Arch::Rdna2 => "AMD RDNA 2 · FSR 3.1 & FSR Frame Gen Supported",
            Arch::OtherAmd => "AMD Radeon · FSR 3.1 Supported",
            Arch::IntelArc => "Intel Arc · XeSS & FSR Supported",
            Arch::Nvidia => "NVIDIA GeForce · Universal Support",
            Arch::Unknown => "Universal Mode",
        }
    }

    pub fn supports_fsr4_int8(&self) -> bool {
        matches!(self, Arch::Rdna3)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub driver_version: String,
    pub arch: Arch,
}

/// Detects the system's display adapter from the Windows registry.
#[cfg(windows)]
pub fn detect_gpu() -> Option<GpuInfo> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn read_sz(key: HKEY, name: &str) -> Option<String> {
        let name_w = wide(name);
        let mut buf = [0u16; 256];
        let mut size: u32 = (buf.len() * 2) as u32;
        let rc = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                name_w.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut _,
                &mut size,
            )
        };
        if rc != 0 {
            return None;
        }
        let n = (size as usize / 2).saturating_sub(1).min(buf.len());
        Some(
            String::from_utf16_lossy(&buf[..n])
                .trim()
                .to_owned(),
        )
    }

    let base = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut best_gpu: Option<GpuInfo> = None;

    for i in 0..32 {
        let sub = wide(&format!("{base}\\{i:04}"));
        let mut key: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            continue;
        }

        let desc = read_sz(key, "DriverDesc").unwrap_or_default();
        let provider = read_sz(key, "ProviderName").unwrap_or_default();
        let ver = read_sz(key, "DriverVersion").unwrap_or_default();
        unsafe { RegCloseKey(key) };

        if desc.is_empty() {
            continue;
        }

        let desc_lower = desc.to_lowercase();
        // Skip virtual display drivers, capture cards, and remote display devices
        if desc_lower.contains("virtual")
            || desc_lower.contains("hyper-v")
            || desc_lower.contains("basic display")
            || desc_lower.contains("remote")
            || desc_lower.contains("iddcx")
        {
            continue;
        }

        let arch = if desc_lower.contains("7900")
            || desc_lower.contains("7800")
            || desc_lower.contains("7700")
            || desc_lower.contains("7600")
            || desc_lower.contains("rdna 3")
            || desc_lower.contains("rdna3")
            || desc_lower.contains("gfx11")
        {
            Arch::Rdna3
        } else if desc_lower.contains("6950")
            || desc_lower.contains("6900")
            || desc_lower.contains("6800")
            || desc_lower.contains("6750")
            || desc_lower.contains("6700")
            || desc_lower.contains("6650")
            || desc_lower.contains("6600")
            || desc_lower.contains("6500")
            || desc_lower.contains("6400")
            || desc_lower.contains("rdna 2")
            || desc_lower.contains("rdna2")
            || desc_lower.contains("gfx10")
        {
            Arch::Rdna2
        } else if desc_lower.contains("radeon") || desc_lower.contains("amd") {
            Arch::OtherAmd
        } else if desc_lower.contains("arc") || desc_lower.contains("intel") {
            Arch::IntelArc
        } else if desc_lower.contains("geforce") || desc_lower.contains("rtx") || desc_lower.contains("gtx") {
            Arch::Nvidia
        } else {
            Arch::Unknown
        };

        let info = GpuInfo {
            name: desc,
            vendor: provider,
            driver_version: ver,
            arch,
        };

        // Prefer discrete AMD or dedicated gaming GPUs over integrated
        if matches!(info.arch, Arch::Rdna3 | Arch::Rdna2) {
            return Some(info);
        }

        if best_gpu.is_none() {
            best_gpu = Some(info);
        }
    }

    best_gpu
}

#[cfg(not(windows))]
pub fn detect_gpu() -> Option<GpuInfo> {
    None
}

/// Sets high-performance GPU preference for a game executable in Windows.
#[cfg(windows)]
pub fn set_high_performance_preference(exe_path: &std::path::Path) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, REG_SZ,
    };

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    let subkey: Vec<u16> = "Software\\Microsoft\\DirectX\\UserGpuPreferences"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut key: HKEY = std::ptr::null_mut();
    let res = unsafe {
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            &mut key,
        )
    };

    if res != 0 {
        return false;
    }

    // GpuPreference=2 specifies "High performance" (dedicated GPU)
    let val_data: Vec<u16> = "GpuPreference=2;"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let name_w = wide(exe_path.as_os_str());
    let write_res = unsafe {
        RegSetValueExW(
            key,
            name_w.as_ptr(),
            0,
            REG_SZ,
            val_data.as_ptr() as *const _,
            (val_data.len() * 2) as u32,
        )
    };

    unsafe { RegCloseKey(key) };
    write_res == 0
}

#[cfg(not(windows))]
pub fn set_high_performance_preference(_exe_path: &std::path::Path) -> bool {
    false
}
