//! Enumeration of the physical keyboards and mice attached to the machine.
//!
//! Multi-keyboard/mouse play assigns one raw-input device to each instance. The setup
//! screens name devices by ordinal ("Keyboard #2"); this module maps those ordinals onto
//! the raw-input handles that ProtoInput selects devices by.

/// Kind of a physical raw-input device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawDeviceKind {
    Keyboard,
    Mouse,
}

/// A physical keyboard or mouse as reported by the OS raw-input layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDevice {
    /// Raw-input device handle (the value ProtoInput selects devices by).
    pub handle: u64,
    /// Device interface path, e.g. `\\?\HID#VID_046D&PID_C52B...`.
    pub name: String,
    /// Keyboard or mouse.
    pub kind: RawDeviceKind,
}

/// Returns whether a device interface path belongs to a remote-session device that no
/// local player can hold.
fn is_virtual_device(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.contains("ROOT#RDP_") || upper.contains("RDP_KBD") || upper.contains("RDP_MOU")
}

/// Orders devices by interface path so "Keyboard #1" names the same device between
/// enumerations, then drops duplicates and virtual devices.
pub fn normalize_devices(mut devices: Vec<RawDevice>) -> Vec<RawDevice> {
    devices.retain(|d| !d.name.is_empty() && !is_virtual_device(&d.name));
    devices.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then(a.handle.cmp(&b.handle))
    });
    devices.dedup_by(|a, b| a.handle == b.handle);
    devices
}

/// Enumerates the keyboards and mice currently attached, in a stable order.
#[cfg(windows)]
pub fn enumerate_raw_devices() -> Vec<RawDevice> {
    use windows::Win32::UI::Input::{
        GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICENAME,
        RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
    };

    let entry_size = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    // SAFETY: a null list pointer asks only for the device count.
    let rc = unsafe { GetRawInputDeviceList(None, &mut count, entry_size) };
    if rc == u32::MAX || count == 0 {
        return Vec::new();
    }
    let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
    // SAFETY: `list` holds `count` entries of `entry_size` bytes each.
    let written = unsafe { GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, entry_size) };
    if written == u32::MAX {
        return Vec::new();
    }
    list.truncate(written as usize);

    let mut devices = Vec::new();
    for entry in list {
        let kind = if entry.dwType == RIM_TYPEKEYBOARD {
            RawDeviceKind::Keyboard
        } else if entry.dwType == RIM_TYPEMOUSE {
            RawDeviceKind::Mouse
        } else {
            continue;
        };
        let mut len = 0u32;
        // SAFETY: a null buffer queries the required length in characters.
        unsafe { GetRawInputDeviceInfoW(entry.hDevice, RIDI_DEVICENAME, None, &mut len) };
        if len == 0 {
            continue;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let mut cap = buf.len() as u32;
        // SAFETY: `buf` has room for `cap` UTF-16 units.
        let got = unsafe {
            GetRawInputDeviceInfoW(
                entry.hDevice,
                RIDI_DEVICENAME,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut cap,
            )
        };
        if got == u32::MAX || got == 0 {
            continue;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        devices.push(RawDevice {
            handle: entry.hDevice.0 as usize as u64,
            name: String::from_utf16_lossy(&buf[..end]),
            kind,
        });
    }
    normalize_devices(devices)
}

/// Enumerates the keyboards and mice currently attached, in a stable order.
#[cfg(not(windows))]
pub fn enumerate_raw_devices() -> Vec<RawDevice> {
    Vec::new()
}

/// Returns the attached devices, re-enumerating at most every two seconds.
///
/// Setup screens call this while rendering, so the OS is not queried on every frame.
pub fn cached_raw_devices() -> Vec<RawDevice> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Vec<RawDevice>)>> = Mutex::new(None);
    let mut guard = match CACHE.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some((at, devices)) = guard.as_ref() {
        if at.elapsed() < Duration::from_secs(2) {
            return devices.clone();
        }
    }
    let devices = enumerate_raw_devices();
    *guard = Some((Instant::now(), devices.clone()));
    devices
}

/// Assignment labels offered for individual (ProtoInput) keyboards and mice: one entry per
/// keyboard, one per mouse, then keyboard+mouse pairs for as many seats as both allow.
///
/// `pair_label` formats a pair from its 1-based seat number.
pub fn proto_assignment_labels(
    devices: &[RawDevice],
    pair_label: impl Fn(usize) -> String,
) -> Vec<String> {
    let keyboards = device_count(devices, RawDeviceKind::Keyboard);
    let mice = device_count(devices, RawDeviceKind::Mouse);
    let mut labels = Vec::with_capacity(keyboards + mice + keyboards.min(mice));
    labels.extend((1..=keyboards).map(|n| format!("Keyboard #{n} (ProtoInput)")));
    labels.extend((1..=mice).map(|n| format!("Mouse #{n} (ProtoInput)")));
    labels.extend((1..=keyboards.min(mice)).map(pair_label));
    labels
}

/// Number of attached devices of `kind`.
pub fn device_count(devices: &[RawDevice], kind: RawDeviceKind) -> usize {
    devices.iter().filter(|d| d.kind == kind).count()
}

/// Returns the raw-input handle of the `ordinal`-th (1-based) device of `kind`.
pub fn nth_device_handle(
    devices: &[RawDevice],
    kind: RawDeviceKind,
    ordinal: usize,
) -> Option<u64> {
    if ordinal == 0 {
        return None;
    }
    devices
        .iter()
        .filter(|d| d.kind == kind)
        .nth(ordinal - 1)
        .map(|d| d.handle)
}

/// Extracts the device ordinal from an assignment label such as
/// `"Keyboard #2 + Mouse #1 (ProtoInput)"` (`label` is `"Keyboard"` or `"Mouse"`).
pub fn ordinal_in_assignment(assignment: &str, label: &str) -> Option<usize> {
    let lower = assignment.to_ascii_lowercase();
    let start = lower.find(&label.to_ascii_lowercase())? + label.len();
    let rest = &assignment[start..];
    let hash = rest.find('#')?;
    // The '#' must belong to this label, not to a later device in the same string.
    if rest[..hash].chars().any(|c| !c.is_whitespace()) {
        return None;
    }
    let digits: String = rest[hash + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<usize>().ok().filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(handle: u64, name: &str, kind: RawDeviceKind) -> RawDevice {
        RawDevice {
            handle,
            name: name.to_string(),
            kind,
        }
    }

    #[test]
    fn ordinals_are_parsed_per_device_label() {
        let a = "Keyboard #2 + Mouse #1 (ProtoInput)";
        assert_eq!(ordinal_in_assignment(a, "Keyboard"), Some(2));
        assert_eq!(ordinal_in_assignment(a, "Mouse"), Some(1));
        assert_eq!(
            ordinal_in_assignment("Keyboard #3 (ProtoInput)", "Mouse"),
            None
        );
        assert_eq!(
            ordinal_in_assignment("Keyboard & Mouse (Paired)", "Keyboard"),
            None
        );
        assert_eq!(ordinal_in_assignment("Gamepad #0", "Keyboard"), None);
    }

    #[test]
    fn devices_are_ordered_filtered_and_indexed_by_kind() {
        let devices = normalize_devices(vec![
            dev(30, r"\\?\HID#VID_B&PID_1#kbd", RawDeviceKind::Keyboard),
            dev(10, r"\\?\HID#VID_A&PID_1#mouse", RawDeviceKind::Mouse),
            dev(20, r"\\?\HID#VID_A&PID_2#kbd", RawDeviceKind::Keyboard),
            dev(40, r"\\?\ROOT#RDP_KBD#0000", RawDeviceKind::Keyboard),
            dev(50, "", RawDeviceKind::Mouse),
            dev(20, r"\\?\HID#VID_A&PID_2#kbd", RawDeviceKind::Keyboard),
        ]);
        assert_eq!(devices.len(), 3);
        assert_eq!(device_count(&devices, RawDeviceKind::Keyboard), 2);
        assert_eq!(
            nth_device_handle(&devices, RawDeviceKind::Keyboard, 1),
            Some(20)
        );
        assert_eq!(
            nth_device_handle(&devices, RawDeviceKind::Keyboard, 2),
            Some(30)
        );
        assert_eq!(
            nth_device_handle(&devices, RawDeviceKind::Keyboard, 3),
            None
        );
        assert_eq!(
            nth_device_handle(&devices, RawDeviceKind::Mouse, 1),
            Some(10)
        );
        assert_eq!(nth_device_handle(&devices, RawDeviceKind::Mouse, 0), None);
    }

    #[test]
    fn assignment_labels_follow_attached_devices() {
        let devices = normalize_devices(vec![
            dev(1, "k1", RawDeviceKind::Keyboard),
            dev(2, "k2", RawDeviceKind::Keyboard),
            dev(3, "k3", RawDeviceKind::Keyboard),
            dev(4, "m1", RawDeviceKind::Mouse),
        ]);
        let labels = proto_assignment_labels(&devices, |n| {
            format!("Keyboard #{n} + Mouse #{n} (ProtoInput)")
        });
        assert_eq!(
            labels,
            vec![
                "Keyboard #1 (ProtoInput)",
                "Keyboard #2 (ProtoInput)",
                "Keyboard #3 (ProtoInput)",
                "Mouse #1 (ProtoInput)",
                "Keyboard #1 + Mouse #1 (ProtoInput)",
            ]
        );
        assert!(proto_assignment_labels(&[], |n| n.to_string()).is_empty());
        // Every generated label resolves back to a device ordinal.
        for label in &labels {
            assert!(
                ordinal_in_assignment(label, "Keyboard").is_some()
                    || ordinal_in_assignment(label, "Mouse").is_some()
            );
        }
    }

    #[test]
    fn enumeration_returns_only_named_devices() {
        for d in enumerate_raw_devices() {
            assert!(!d.name.is_empty());
        }
    }
}
