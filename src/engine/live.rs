//! The sound card side of a live engine: which devices there are, how one is asked to run, and the streams that play to it and hear from it.

use super::clock::Timing;
use super::load::Meter;
use super::mixer::{CAPACITY, Command, Garbage, Input, Mixer, Voices};
use super::report::{self, Hub, Report, Tapped};
use crate::error::Error;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// An output or input device, with every channel count, sample rate and buffer size it supports.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    /// The name [`Settings`] picks the device by.
    pub name: String,
    /// Whether it is an output device, and not an input one.
    pub output: bool,
    /// Each channel count it supports.
    pub channels: Vec<u16>,
    /// Each a lowest and highest rate, in frames a second.
    pub rates: Vec<(u32, u32)>,
    /// The fewest and most frames a buffer can hold, unless the device does not say.
    pub buffers: Option<(u32, u32)>,
}

/// How a live engine starts, for [`Engine::live_with`](crate::Engine::live_with); what is left out falls back to the device's defaults.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// The output device, by name; the default output when none is given.
    pub device: Option<String>,
    /// Whether to read an input device, at the output's rate.
    pub with_input: bool,
    /// The input device, by name; the default input when none is given.
    pub input_device: Option<String>,
    /// The rate to run at, in frames a second; the device's default when none is given.
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

type Streams = (Option<cpal::Stream>, cpal::Stream);

pub(super) fn open(
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Garbage>,
    clock: Arc<AtomicU64>,
    voices: Arc<Voices>,
    meter: Arc<Meter>,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    hub: Arc<Hub>,
    settings: &Settings,
) -> Result<(Streams, Opened), String> {
    let host = cpal::default_host();
    let device = output(&host, settings.device.as_deref())?;
    let name = name_of(&device);
    let rate = match settings.rate {
        Some(rate) => rate,
        None => device
            .default_output_config()
            .map_err(|e| e.to_string())?
            .sample_rate(),
    };
    let ranges: Vec<Range> = device
        .supported_output_configs()
        .map_err(|e| e.to_string())?
        .map(Range::from)
        .collect();
    let chosen = choose(&name, &ranges, rate, settings.buffer)?;
    let (input_stream, input) = if settings.with_input {
        open_input(&host, settings.input_device.as_deref(), chosen.rate)?
    } else {
        (None, None)
    };
    let inputs = input.as_ref().map_or(0, |i| i.channels);
    let frames = clock.clone();
    let following = hub.clone();
    let (tap, tapped) = report::tap(hub, chosen.rate as f64);
    let mixer = Mixer::new(commands, garbage, clock, voices, input, tap);
    let timing = Arc::new(Mutex::new(Timing::new(chosen.rate as f64)));
    let live = Live {
        mixer,
        meter,
        rate: chosen.rate as f64,
        failed,
        lost,
        frames,
        timing: timing.clone(),
        hub: following,
    };
    let config = cpal::StreamConfig {
        channels: chosen.channels,
        sample_rate: chosen.rate,
        buffer_size: chosen
            .buffer
            .map_or(cpal::BufferSize::Default, cpal::BufferSize::Fixed),
    };
    let stream = match chosen.format {
        Format::F32 => output_stream::<f32>(&device, config, live),
        Format::I24 => output_stream::<cpal::I24>(&device, config, live),
        Format::I16 => output_stream::<i16>(&device, config, live),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    let waiting = Instant::now();
    while timing
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .system(0.0)
        .is_none()
    {
        if waiting.elapsed() > Duration::from_secs(2) {
            return Err(format!(
                "{name} started but asked for no audio within 2 seconds"
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok((
        (input_stream, stream),
        Opened {
            rate: chosen.rate as f64,
            inputs,
            name,
            timing,
            tapped,
        },
    ))
}

/// What a live engine learns once its device is playing.
pub(super) struct Opened {
    pub(super) rate: f64,
    pub(super) inputs: usize,
    pub(super) name: String,
    pub(super) timing: Arc<Mutex<Timing>>,
    pub(super) tapped: Tapped,
}

/// What the output callback owns.
struct Live {
    mixer: Mixer,
    meter: Arc<Meter>,
    rate: f64,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
    timing: Arc<Mutex<Timing>>,
    hub: Arc<Hub>,
}

fn output_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    live: Live,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let Live {
        mut mixer,
        meter,
        rate,
        failed,
        lost,
        frames,
        timing,
        hub,
    } = live;
    let channels = config.channels as usize;
    let (moving, moved_frames) = (timing.clone(), frames.clone());
    let mut left = vec![0.0f32; 16384];
    let mut right = vec![0.0f32; 16384];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let stamp = info.timestamp();
                let latency = stamp.playback.duration_since(stamp.callback).as_secs_f64();
                let system = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                // The callback never waits on the engine's thread; a reading missed while it holds the lock is one of hundreds a second.
                if let Ok(mut timing) = timing.try_lock() {
                    timing.observe(frames.load(Ordering::Acquire), system, Some(latency));
                }
                let began = Instant::now();
                fill(&mut mixer, &failed, data, channels, &mut left, &mut right);
                meter.record(began.elapsed(), data.len() / channels, rate, true);
            },
            move |e| match e.kind() {
                cpal::ErrorKind::DeviceNotAvailable => lost.store(true, Ordering::Release),
                cpal::ErrorKind::DeviceChanged => {
                    follow(hub.clone(), moving.clone(), moved_frames.clone(), rate)
                }
                _ => eprintln!("mxs output: {e}"),
            },
            None,
        )
        .map_err(|e| e.to_string())
}

/// Once the engine has played a tenth of a second on the system's new default output, lines its clock up with the system's again and reports the new output.
// The move holds the callbacks up for about 100 ms, and the clock otherwise rises to meet that by a millisecond a second.
fn follow(hub: Arc<Hub>, timing: Arc<Mutex<Timing>>, frames: Arc<AtomicU64>, rate: f64) {
    std::thread::spawn(move || {
        let from = frames.load(Ordering::Acquire) + (rate / 10.0) as u64;
        let waiting = Instant::now();
        while frames.load(Ordering::Acquire) < from {
            if waiting.elapsed() > Duration::from_secs(2) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut held = timing.lock().unwrap_or_else(|e| e.into_inner());
        held.restart();
        let delay_ms = held.latency().map(|seconds| seconds * 1000.0);
        drop(held);
        let seconds = frames.load(Ordering::Acquire) as f64 / rate;
        let device = output(&cpal::default_host(), None).ok();
        let name = device.as_ref().map(name_of);
        hub.keep(Report::Output {
            seconds,
            device: name.clone(),
            rate,
            delay_ms,
        });
        let theirs = device
            .and_then(|device| device.default_output_config().ok())
            .map(|config| config.sample_rate() as f64)
            .filter(|theirs| *theirs != rate);
        if let (Some(name), Some(theirs)) = (name, theirs) {
            hub.warn(
                seconds,
                format!("{name} plays at {theirs} Hz and the engine at {rate} Hz, so the system converts between them"),
            );
        }
    });
}

fn open_input(
    host: &cpal::Host,
    name: Option<&str>,
    rate: cpal::SampleRate,
) -> Result<(Option<cpal::Stream>, Option<Input>), String> {
    let device = input(host, name)?;
    let ranges = device
        .supported_input_configs()
        .map_err(|e| e.to_string())?;
    let config = ranges
        .filter(|range| range.sample_format() == cpal::SampleFormat::F32)
        .find_map(|range| range.try_with_sample_rate(rate));
    let Some(config) = config else {
        let found = device
            .default_input_config()
            .map_err(|e| e.to_string())?
            .sample_rate();
        return Err(format!(
            "the input runs at {found} Hz and the output at {rate} Hz"
        ));
    };
    let channels = config.channels() as usize;
    let (mut feed, frames) = rtrb::RingBuffer::new(CAPACITY * channels);
    let stream = device
        .build_input_stream(
            config.into(),
            move |data: &[f32], _| {
                for &x in data {
                    let _ = feed.push(x);
                }
            },
            |e| eprintln!("mxs input: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((Some(stream), Some(Input { frames, channels })))
}

/// Fills an interleaved buffer of `channels` from the mixer, or silence once it has panicked, which marks the engine failed instead of aborting the process.
fn fill<T: cpal::Sample + cpal::FromSample<f32>>(
    mixer: &mut Mixer,
    failed: &AtomicBool,
    data: &mut [T],
    channels: usize,
    left: &mut [f32],
    right: &mut [f32],
) {
    if failed.load(Ordering::Relaxed) {
        data.fill(T::EQUILIBRIUM);
        return;
    }
    let filled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for frames in data.chunks_mut(channels * left.len()) {
            let n = frames.len() / channels;
            mixer.render(&mut left[..n], &mut right[..n]);
            for (i, frame) in frames.chunks_mut(channels).enumerate() {
                frame[0] = T::from_sample(left[i]);
                if channels > 1 {
                    frame[1] = T::from_sample(right[i]);
                }
                frame.iter_mut().skip(2).for_each(|x| *x = T::EQUILIBRIUM);
            }
        }
    }));
    if filled.is_err() {
        failed.store(true, Ordering::Release);
        data.fill(T::EQUILIBRIUM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, Output};
    use crate::{Event, Note, SynthDecl};
    use fundsp::prelude::*;

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

    #[test]
    fn a_panic_while_filling_audio_silences_the_engine_and_is_reported() {
        let mut engine = Engine::offline(48_000.0);
        engine.register(SynthDecl::new("broken").build(|_, _| {
            let breaks =
                map(|_: &Frame<f32, U0>| -> f32 { panic!("on purpose while filling audio") });
            Note::rings(breaks, 1.0)
        }));
        engine
            .play(&[(0.0, Event::note(None, "broken", &[]))])
            .unwrap();
        let Output::Offline(mixer) = &mut engine.output else {
            unreachable!()
        };
        let failed = AtomicBool::new(false);
        let mut data = vec![1.0f32; 256];
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        fill(mixer, &failed, &mut data, 2, &mut left, &mut right);
        assert!(failed.load(Ordering::Acquire));
        assert!(data.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn a_device_of_16_bit_samples_gets_the_float_samples_rounded_to_them() {
        let render = || {
            let mut engine = Engine::offline(48_000.0);
            crate::synths::all()
                .into_iter()
                .for_each(|s| engine.register(s));
            engine
                .play(&[(0.0, Event::note(None, "saw", &[("amp", 2.0)]))])
                .unwrap();
            engine
        };
        let (mut floats, mut ints) = (render(), render());
        let (Output::Offline(a), Output::Offline(b)) = (&mut floats.output, &mut ints.output)
        else {
            unreachable!()
        };
        let failed = AtomicBool::new(false);
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        let mut float_data = vec![0.0f32; 2 * 4800];
        let mut int_data = vec![0i16; 2 * 4800];
        fill(a, &failed, &mut float_data, 2, &mut left, &mut right);
        fill(b, &failed, &mut int_data, 2, &mut left, &mut right);
        assert!(float_data.iter().any(|x| x.abs() > 0.1));
        for (x, n) in float_data.iter().zip(&int_data) {
            assert!((x * 32768.0 - *n as f32).abs() <= 1.0, "{x} as {n}");
        }
    }
}
