//! Reading the output of `pactl` and `wpctl` (Linux backend). Pure functions,
//! so they are tested on every platform. Commands run with `LC_ALL=C`, so the
//! field names are always in English.

/// First percentage in `pactl get-sink-volume`:
/// "Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: …" → 40.
pub fn volume(out: &str) -> Option<u32> {
    let end = out.find('%')?;
    let digits: String = out[..end].chars().rev().take_while(char::is_ascii_digit).collect();
    let n: u32 = digits.chars().rev().collect::<String>().parse().ok()?;
    Some(n.min(100))
}

/// `pactl get-sink-mute`: "Mute: yes" → true.
pub fn mute(out: &str) -> Option<bool> {
    let rest = out.lines().find_map(|l| l.trim().strip_prefix("Mute:"))?;
    match rest.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// `pactl list sinks` → (name, description) of each output.
pub fn sinks(out: &str) -> Vec<(String, String)> {
    let mut list = Vec::new();
    let mut name: Option<String> = None;
    for line in out.lines() {
        let line = line.trim();
        if line.starts_with("Sink #") {
            name = None;
        } else if let Some(n) = line.strip_prefix("Name:") {
            name = Some(n.trim().to_string());
        } else if let Some(d) = line.strip_prefix("Description:") {
            if let Some(n) = name.take() {
                list.push((n, d.trim().to_string()));
            }
        }
    }
    list
}

/// `wpctl get-volume @DEFAULT_AUDIO_SINK@`: "Volume: 0.40 [MUTED]" → (40, true).
pub fn wpctl_volume(out: &str) -> Option<(u32, bool)> {
    let rest = out.trim().strip_prefix("Volume:")?.trim();
    let level: f64 = rest.split_whitespace().next()?.parse().ok()?;
    Some((((level * 100.0).round() as u32).min(100), rest.contains("[MUTED]")))
}

/// `wpctl inspect @DEFAULT_AUDIO_SINK@`: the `node.description = "…"` line.
pub fn wpctl_description(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let l = l.trim().trim_start_matches('*').trim();
        let v = l.strip_prefix("node.description")?.trim().strip_prefix('=')?.trim();
        Some(v.trim_matches('"').to_string())
    })
}

/// Backlight to use from (name, type) of /sys/class/backlight/*: the firmware
/// (ACPI) one first, then the platform one, then the raw GPU one.
pub fn pick_backlight(devices: &[(String, String)]) -> Option<String> {
    ["firmware", "platform", "raw"]
        .iter()
        .find_map(|kind| devices.iter().find(|(_, t)| t.trim() == *kind))
        .or(devices.first())
        .map(|(n, _)| n.clone())
}

/// Brightness as 0–100 from the raw value.
pub fn percent(raw: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    ((raw as u64 * 100 + max as u64 / 2) / max as u64).min(100) as u32
}

/// Raw value for a 0–100 brightness; never 0, which turns some panels off.
pub fn raw(percent: u32, max: u32) -> u32 {
    ((percent.min(100) as u64 * max as u64 + 50) / 100).max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pactl_volume() {
        let out = "Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: 26214 /  40% / -23.88 dB\n        balance 0.00\n";
        assert_eq!(volume(out), Some(40));
        assert_eq!(volume("Volume: mono: 98304 / 150% / 10.57 dB"), Some(100));
        assert_eq!(volume("Volume: mono: 0 /   0% / -inf dB"), Some(0));
        assert_eq!(volume("nothing here"), None);
    }

    #[test]
    fn reads_pactl_mute() {
        assert_eq!(mute("Mute: yes\n"), Some(true));
        assert_eq!(mute("Mute: no"), Some(false));
        assert_eq!(mute(""), None);
    }

    #[test]
    fn reads_sinks() {
        let out = "Sink #56\n\tState: RUNNING\n\tName: alsa_output.pci-0000_00_1f.3.analog-stereo\n\tDescription: Built-in Audio Analog Stereo\n\tDriver: PipeWire\n\nSink #71\n\tName: bluez_output.AA_BB.1\n\tDescription: WH-1000XM4\n";
        assert_eq!(
            sinks(out),
            vec![
                ("alsa_output.pci-0000_00_1f.3.analog-stereo".into(), "Built-in Audio Analog Stereo".into()),
                ("bluez_output.AA_BB.1".into(), "WH-1000XM4".into()),
            ]
        );
        assert!(sinks("").is_empty());
    }

    #[test]
    fn reads_wpctl() {
        assert_eq!(wpctl_volume("Volume: 0.40\n"), Some((40, false)));
        assert_eq!(wpctl_volume("Volume: 0.55 [MUTED]"), Some((55, true)));
        assert_eq!(wpctl_volume("Volume: 1.20"), Some((100, false)));
        assert_eq!(wpctl_volume("error"), None);
        let inspect = "id 56, type PipeWire:Interface:Node\n    media.class = \"Audio/Sink\"\n  * node.description = \"Built-in Audio Analog Stereo\"\n  * node.name = \"alsa_output\"\n";
        assert_eq!(wpctl_description(inspect).as_deref(), Some("Built-in Audio Analog Stereo"));
    }

    #[test]
    fn picks_backlight() {
        let d = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        assert_eq!(pick_backlight(&d(&[("amdgpu_bl1", "raw"), ("acpi_video0", "firmware\n")])).as_deref(), Some("acpi_video0"));
        assert_eq!(pick_backlight(&d(&[("intel_backlight", "raw")])).as_deref(), Some("intel_backlight"));
        assert_eq!(pick_backlight(&[]), None);
    }

    #[test]
    fn converts_brightness() {
        assert_eq!(percent(96000, 96000), 100);
        assert_eq!(percent(48000, 96000), 50);
        assert_eq!(percent(1, 0), 0);
        assert_eq!(raw(50, 255), 128);
        assert_eq!(raw(0, 255), 1);
        assert_eq!(raw(100, 96000), 96000);
    }
}
