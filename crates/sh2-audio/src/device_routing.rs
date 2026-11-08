//! Platform audio device enumeration, CPAL endpoint selection, and PipeWire/PulseAudio live sink migration.

use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::Mutex;

/// Global mutex serializing `cpal` host/device queries across threads.
///
/// On Windows WASAPI, concurrent `cpal::default_host().output_devices()` calls from
/// parallel threads (e.g., during multi-threaded `cargo test` execution) can race inside
/// `IMMDeviceEnumerator` COM initialization/uninitialization (`STATUS_ACCESS_VIOLATION`).
static CPAL_HOST_ENUM_LOCK: Mutex<()> = Mutex::new(());

/// Returns `true` if `name` is a virtual/software ALSA rate-converter or upmix filter plugin
/// that should be hidden from user-facing device lists and ignored as a hardware target.
#[inline]
pub fn is_filtered_alsa_plugin_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("speex")
        || lower.contains("rate")
        || lower.contains("samplerate")
        || lower.contains("upmix")
        || lower.contains("vdownmix")
}

/// Enumerate all currently available audio output device names on the host system.
///
/// Resolves human-readable device names (e.g. "MiniFuse 2", "Headphones", "HDMI") via PulseAudio / PipeWire
/// when available on Linux desktop systems, cleanly filtering out internal ALSA DSP plugins like "speex",
/// "speexrate", "samplerate", or "upmix".
pub fn enumerate_output_devices() -> Vec<String> {
    let mut devices = Vec::new();

    // 1. Attempt querying PulseAudio / PipeWire sinks for friendly display descriptions
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("pactl")
            .args(["-f", "json", "list", "sinks"])
            .output()
        {
            if output.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(sinks) = val.as_array() {
                        for sink in sinks {
                            let name = sink.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let desc = sink
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or(name);

                            if is_virtual_loopback_sink(name, desc) {
                                continue;
                            }

                            let clean_name = desc.trim().to_string();
                            if !clean_name.is_empty() && !devices.contains(&clean_name) {
                                devices.push(clean_name);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Query CPAL host devices if pactl is unavailable or returns no sinks
    if devices.is_empty() {
        let _guard = CPAL_HOST_ENUM_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let host = cpal::default_host();
        if let Ok(iter) = host.output_devices() {
            for device in iter {
                if let Ok(name) = device.name() {
                    if is_filtered_alsa_plugin_name(&name) {
                        continue;
                    }
                    if !devices.contains(&name) {
                        devices.push(name);
                    }
                }
            }
        }
    }

    devices
}

/// Sanitizes an optional user-configured `device_name` (dropping legacy ALSA rate plugins)
/// and selects the matching [`cpal::Device`] (or PipeWire/PulseAudio/default fallback).
pub fn select_cpal_output_device(
    device_name: Option<&str>,
) -> (Option<&str>, Option<cpal::Device>) {
    let clean_device_name = match device_name {
        Some(name) if is_filtered_alsa_plugin_name(name) => {
            tracing::warn!(
                "Ignoring invalid/legacy ALSA DSP plugin device name: '{name}'; using system default."
            );
            None
        }
        other => other,
    };

    let _guard = CPAL_HOST_ENUM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let host = cpal::default_host();
    let device = if let Some(target_name) = clean_device_name {
        let mut found = None;
        if let Ok(iter) = host.output_devices() {
            for dev in iter {
                if let Ok(name) = dev.name() {
                    if name == target_name {
                        found = Some(dev);
                        break;
                    }
                }
            }
        }
        if found.is_none() {
            found = host.output_devices().ok().and_then(|mut iter| {
                iter.find(|d| {
                    d.name()
                        .map(|n| n == "pipewire" || n == "pulse" || n == "default")
                        .unwrap_or(false)
                })
            });
        }
        found.or_else(|| host.default_output_device())
    } else {
        let pulse_or_pipewire = host.output_devices().ok().and_then(|mut iter| {
            iter.find(|d| {
                d.name()
                    .map(|n| n == "pipewire" || n == "pulse")
                    .unwrap_or(false)
            })
        });
        pulse_or_pipewire.or_else(|| host.default_output_device())
    };

    (clean_device_name, device)
}

#[cfg(target_os = "linux")]
#[inline]
fn is_virtual_loopback_sink(name: &str, desc: &str) -> bool {
    name.contains("Loopback")
        || desc.contains("Loopback")
        || name.contains("spotifast")
        || desc.contains("Soloist")
        || desc.contains("monitor")
}

/// Migrates the active running game audio stream to the requested device in real-time.
///
/// On Linux with PipeWire/PulseAudio, this uses `pactl move-sink-input` to immediately
/// move the existing ALSA/PipeWire stream to the requested hardware sink without destroying
/// the audio manager or interrupting audio playback.
pub fn migrate_stream_to_device(_device_name: Option<&str>) {
    #[cfg(target_os = "linux")]
    {
        let target_sink_name = resolve_linux_target_sink(_device_name);
        let Some(sink) = target_sink_name else {
            return;
        };

        let my_pid = std::process::id();
        let my_pid_str = my_pid.to_string();
        let client_pids = query_pactl_client_pids();

        let Ok(input_output) = std::process::Command::new("pactl")
            .args(["-f", "json", "list", "sink-inputs"])
            .output()
        else {
            return;
        };
        let Ok(input_val) = serde_json::from_slice::<serde_json::Value>(&input_output.stdout)
        else {
            return;
        };
        let Some(inputs) = input_val.as_array() else {
            return;
        };

        for input in inputs {
            let idx = input.get("index").and_then(|i| i.as_u64());
            let props = input.get("properties");

            let input_pid = props
                .and_then(|p| p.get("application.process.id"))
                .and_then(|pid| pid.as_str());
            let client_id_owned = input
                .get("client")
                .and_then(|c| c.as_str().map(|s| s.to_string()))
                .or_else(|| {
                    input
                        .get("client")
                        .and_then(|c| c.as_u64())
                        .map(|u| u.to_string())
                });
            let client_pid = client_id_owned
                .as_deref()
                .and_then(|cid| client_pids.get(cid));

            let app = props
                .and_then(|p| p.get("application.name"))
                .and_then(|a| a.as_str())
                .unwrap_or("");

            let is_our_process = input_pid == Some(&my_pid_str)
                || client_pid.map(|p| p.as_str()) == Some(&my_pid_str)
                || app.to_lowercase().contains("sh2");

            if is_our_process {
                if let Some(i) = idx {
                    let res = std::process::Command::new("pactl")
                        .args(["move-sink-input", &i.to_string(), &sink])
                        .status();
                    tracing::info!(
                        "[Audio] Live migrated stream #{} (PID {}) to sink '{}' (success: {:?})",
                        i,
                        my_pid,
                        sink,
                        res.map(|s| s.success()).unwrap_or(false)
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn resolve_linux_target_sink(device_name: Option<&str>) -> Option<String> {
    let Some(target) = device_name else {
        return std::process::Command::new("pactl")
            .arg("get-default-sink")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());
    };

    let out = std::process::Command::new("pactl")
        .args(["-f", "json", "list", "sinks"])
        .output()
        .ok()?;
    let val = serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()?;
    let sinks = val.as_array()?;
    let target_clean = target.trim();

    // 1. Exact match on description or name (ignoring loopback/virtual)
    for sink in sinks {
        let name = sink.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let desc = sink
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or(name);
        if is_virtual_loopback_sink(name, desc) && !target_clean.contains("Loopback") {
            continue;
        }
        if desc.trim().eq_ignore_ascii_case(target_clean)
            || name.trim().eq_ignore_ascii_case(target_clean)
        {
            return Some(name.to_string());
        }
    }

    // 2. Fuzzy / token-based match (e.g. DualSense, MiniFuse 2)
    let target_lower = target_clean.to_lowercase();
    for sink in sinks {
        let name = sink.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let desc = sink
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or(name);
        if is_virtual_loopback_sink(name, desc) && !target_clean.contains("Loopback") {
            continue;
        }
        let props = sink.get("properties");
        let dev_desc = props
            .and_then(|p| p.get("device.description"))
            .and_then(|d| d.as_str())
            .unwrap_or("");
        let nick = props
            .and_then(|p| p.get("node.nick"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let card_name = props
            .and_then(|p| p.get("alsa.card_name"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        let haystack = format!("{name} {desc} {dev_desc} {nick} {card_name}").to_lowercase();
        if target_lower
            .split_whitespace()
            .any(|tok| tok.len() > 3 && haystack.contains(tok))
        {
            return Some(name.to_string());
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn query_pactl_client_pids() -> std::collections::HashMap<String, String> {
    let mut client_pids = std::collections::HashMap::new();
    if let Ok(clients_out) = std::process::Command::new("pactl")
        .args(["-f", "json", "list", "clients"])
        .output()
    {
        if let Ok(clients_val) = serde_json::from_slice::<serde_json::Value>(&clients_out.stdout) {
            if let Some(clients) = clients_val.as_array() {
                for client in clients {
                    let c_idx = client.get("index").and_then(|i| i.as_u64());
                    let c_pid = client
                        .get("properties")
                        .and_then(|p| p.get("application.process.id"))
                        .and_then(|pid| pid.as_str());
                    if let (Some(idx), Some(pid)) = (c_idx, c_pid) {
                        client_pids.insert(idx.to_string(), pid.to_string());
                    }
                }
            }
        }
    }
    client_pids
}
