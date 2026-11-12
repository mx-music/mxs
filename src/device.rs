//! The sound card side of a live engine: which devices there are, and how one is asked to run.

use crate::engine::Error;
use cpal::traits::{DeviceTrait, HostTrait};

/// An output or input device, with every channel count, sample rate and buffer size it supports.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub output: bool,
    pub channels: Vec<u16>,
    /// Each a lowest and highest rate, in frames a second.
    pub rates: Vec<(u32, u32)>,
    /// The fewest and most frames a buffer can hold, unless the device does not say.
    pub buffers: Option<(u32, u32)>,
}

/// How a live engine starts; what is left out falls back to the device's defaults.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// The output device, by name; the default output when none is given.
    pub device: Option<String>,
    /// Whether to read an input device, at the output's rate.
    pub with_input: bool,
    /// The input device, by name; the default input when none is given.
    pub input_device: Option<String>,
    pub rate: Option<u32>,
    /// The frames the device asks for at a time.
    pub buffer: Option<u32>,
}

/// The output and input devices of the default host.
pub fn devices() -> Result<Vec<Device>, Error> {
    let host = cpal::default_host();
    let mut found = Vec::new();
    for device in host
        .output_devices()
        .map_err(|e| Error::Device(e.to_string()))?
    {
        let ranges = device
            .supported_output_configs()
            .map_err(|e| Error::Device(e.to_string()))?;
        found.push(listed(
            name_of(&device),
            true,
            &ranges.map(Range::from).collect::<Vec<_>>(),
        ));
    }
    for device in host
        .input_devices()
        .map_err(|e| Error::Device(e.to_string()))?
    {
        let ranges = device
            .supported_input_configs()
            .map_err(|e| Error::Device(e.to_string()))?;
        found.push(listed(
            name_of(&device),
            false,
            &ranges.map(Range::from).collect::<Vec<_>>(),
        ));
    }
    Ok(found)
}

pub(crate) fn name_of(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "an unnamed device".to_string())
}

fn listed(name: String, output: bool, ranges: &[Range]) -> Device {
    let mut channels: Vec<u16> = ranges.iter().map(|r| r.channels).collect();
    channels.sort();
    channels.dedup();
    let mut rates: Vec<(u32, u32)> = ranges.iter().map(|r| r.rates).collect();
    rates.sort();
    rates.dedup();
    let buffers = ranges
        .iter()
        .filter_map(|r| r.buffers)
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
    Device {
        name,
        output,
        channels,
        rates,
        buffers,
    }
}

/// The output device named, or the default one.
pub(crate) fn output(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device, String> {
    match name {
        None => host
            .default_output_device()
            .ok_or_else(|| "no output device".to_string()),
        Some(name) => find(
            host.output_devices().map_err(|e| e.to_string())?.collect(),
            name,
            "output",
        ),
    }
}

/// The input device named, or the default one.
pub(crate) fn input(host: &cpal::Host, name: Option<&str>) -> Result<cpal::Device, String> {
    match name {
        None => host
            .default_input_device()
            .ok_or_else(|| "no input device".to_string()),
        Some(name) => find(
            host.input_devices().map_err(|e| e.to_string())?.collect(),
            name,
            "input",
        ),
    }
}

fn find(devices: Vec<cpal::Device>, name: &str, kind: &str) -> Result<cpal::Device, String> {
    let names: Vec<String> = devices.iter().map(name_of).collect();
    match names.iter().position(|n| n == name) {
        Some(i) => Ok(devices.into_iter().nth(i).expect("a device for each name")),
        None => {
            let known = names.join(", ");
            Err(format!(
                "no {kind} device named {name}; the {kind}s are {known}"
            ))
        }
    }
}

/// The sample types the engine writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Format {
    F32,
    I24,
    I16,
}

/// One way a device can run, as cpal lists it; `format` is None for a sample type the engine does not write.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Range {
    pub channels: u16,
    pub rates: (u32, u32),
    pub buffers: Option<(u32, u32)>,
    pub format: Option<Format>,
}

impl From<cpal::SupportedStreamConfigRange> for Range {
    fn from(range: cpal::SupportedStreamConfigRange) -> Range {
        let format = match range.sample_format() {
            cpal::SampleFormat::F32 => Some(Format::F32),
            cpal::SampleFormat::I24 => Some(Format::I24),
            cpal::SampleFormat::I16 => Some(Format::I16),
            _ => None,
        };
        let buffers = match range.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => Some((*min, *max)),
            cpal::SupportedBufferSize::Unknown => None,
        };
        Range {
            channels: range.channels(),
            rates: (range.min_sample_rate(), range.max_sample_rate()),
            buffers,
            format,
        }
    }
}

/// How the device runs: its channels, rate, buffer size and sample type.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chosen {
    pub channels: u16,
    pub rate: u32,
    pub buffer: Option<u32>,
    pub format: Format,
}

/// Picks the way `ranges` can run at `rate` with `buffer` frames, preferring float samples and then stereo, or says what the device supports instead.
pub(crate) fn choose(
    name: &str,
    ranges: &[Range],
    rate: u32,
    buffer: Option<u32>,
) -> Result<Chosen, String> {
    let usable: Vec<&Range> = ranges.iter().filter(|r| r.format.is_some()).collect();
    if usable.is_empty() {
        return Err(format!(
            "{name} takes none of 32-bit float, 24-bit or 16-bit samples"
        ));
    }
    let mut fitting: Vec<&Range> = usable
        .iter()
        .copied()
        .filter(|r| r.rates.0 <= rate && rate <= r.rates.1)
        .collect();
    if fitting.is_empty() {
        let rates = spans(usable.iter().map(|r| r.rates));
        return Err(format!("{name} runs at {rates} Hz, not {rate}"));
    }
    fitting.sort_by_key(|r| (r.format, r.channels < 2, r.channels));
    let range = fitting[0];
    if let (Some(frames), Some((least, most))) = (buffer, range.buffers)
        && !(least..=most).contains(&frames)
    {
        return Err(format!(
            "{name} takes buffers of {least} to {most} frames, not {frames}"
        ));
    }
    Ok(Chosen {
        channels: range.channels,
        rate,
        buffer,
        format: range.format.expect("a usable format"),
    })
}

fn spans(rates: impl Iterator<Item = (u32, u32)>) -> String {
    let mut rates: Vec<(u32, u32)> = rates.collect();
    rates.sort();
    rates.dedup();
    let shown: Vec<String> = rates
        .iter()
        .map(|&(low, high)| {
            if low == high {
                low.to_string()
            } else {
                format!("{low} to {high}")
            }
        })
        .collect();
    shown.join(" or ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(
        channels: u16,
        rates: (u32, u32),
        buffers: (u32, u32),
        format: Option<Format>,
    ) -> Range {
        Range {
            channels,
            rates,
            buffers: Some(buffers),
            format,
        }
    }

    #[test]
    fn a_buffer_the_device_cannot_take_is_refused_naming_the_sizes_it_can() {
        let ranges = [range(2, (44_100, 48_000), (14, 4096), Some(Format::F32))];
        assert_eq!(
            choose("Speakers", &ranges, 48_000, Some(7)),
            Err("Speakers takes buffers of 14 to 4096 frames, not 7".to_string())
        );
        assert_eq!(
            choose("Speakers", &ranges, 48_000, Some(128))
                .unwrap()
                .buffer,
            Some(128)
        );
    }

    #[test]
    fn a_rate_the_device_cannot_run_at_is_refused_naming_the_rates_it_can() {
        let ranges = [
            range(2, (44_100, 44_100), (14, 4096), Some(Format::F32)),
            range(2, (48_000, 48_000), (14, 4096), Some(Format::F32)),
        ];
        assert_eq!(
            choose("Speakers", &ranges, 22_050, None),
            Err("Speakers runs at 44100 or 48000 Hz, not 22050".to_string())
        );
    }

    #[test]
    fn float_samples_are_preferred_then_stereo_and_16_bit_ones_are_taken_when_they_are_all_there_is()
     {
        let ranges = [
            range(2, (48_000, 48_000), (32, 1024), Some(Format::I16)),
            range(8, (48_000, 48_000), (32, 1024), Some(Format::F32)),
            range(2, (48_000, 48_000), (32, 1024), Some(Format::F32)),
            range(1, (48_000, 48_000), (32, 1024), Some(Format::F32)),
        ];
        let chosen = choose("Interface", &ranges, 48_000, None).unwrap();
        assert_eq!((chosen.format, chosen.channels), (Format::F32, 2));
        let chosen = choose("Interface", &ranges[..1], 48_000, None).unwrap();
        assert_eq!((chosen.format, chosen.channels), (Format::I16, 2));
    }

    #[test]
    fn a_device_of_sample_types_the_engine_does_not_write_is_refused() {
        let ranges = [range(2, (48_000, 48_000), (32, 1024), None)];
        assert_eq!(
            choose("Old", &ranges, 48_000, None),
            Err("Old takes none of 32-bit float, 24-bit or 16-bit samples".to_string())
        );
    }

    #[test]
    fn a_listed_device_gathers_the_channels_rates_and_buffers_of_its_ranges() {
        let ranges = [
            range(2, (44_100, 48_000), (14, 512), Some(Format::F32)),
            range(1, (44_100, 48_000), (32, 4096), Some(Format::I16)),
        ];
        let device = listed("Speakers".to_string(), true, &ranges);
        assert_eq!(
            device,
            Device {
                name: "Speakers".to_string(),
                output: true,
                channels: vec![1, 2],
                rates: vec![(44_100, 48_000)],
                buffers: Some((14, 4096))
            }
        );
    }
}
