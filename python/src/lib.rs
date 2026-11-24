//! mxs for Python: an engine, live or offline, with mxs's synths and effects and those read from files.

use engine::{Event, Settings, Target, Value, effects, synths};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

create_exception!(
    mxs,
    Error,
    PyException,
    "An error mxs reports, carrying its message."
);

fn raise(error: impl std::fmt::Display) -> PyErr {
    Error::new_err(error.to_string())
}

/// The options given by name, each a number or, for an option that takes names, text.
fn options(given: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<(String, Value)>> {
    let Some(given) = given else {
        return Ok(Vec::new());
    };
    given
        .iter()
        .map(|(key, value)| {
            let key: String = key.extract()?;
            let value = match value.extract::<f64>() {
                Ok(number) => Value::Number(number),
                Err(_) => Value::Name(value.extract::<String>()?),
            };
            Ok((key, value))
        })
        .collect()
}

fn numbers(given: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<(String, f64)>> {
    let Some(given) = given else {
        return Ok(Vec::new());
    };
    given
        .iter()
        .map(|(key, value)| Ok((key.extract()?, value.extract()?)))
        .collect()
}

/// Samples as a Python `array` of 32-bit floats.
fn array<'py>(py: Python<'py>, samples: &[f32]) -> PyResult<Bound<'py, PyAny>> {
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_ne_bytes()).collect();
    py.import("array")?
        .getattr("array")?
        .call1(("f", PyBytes::new(py, &bytes)))
}

/// An engine with mxs's synths and effects registered: live on a sound card, or offline, rendering when asked.
#[pyclass(unsendable, module = "mxs")]
struct Engine {
    engine: engine::Engine,
}

impl Engine {
    fn with_everything(mut engine: engine::Engine) -> Engine {
        synths::all().into_iter().for_each(|s| engine.register(s));
        effects::all()
            .into_iter()
            .for_each(|e| engine.register_effect(e));
        Engine { engine }
    }

    fn play(&mut self, at: f64, event: Event) -> PyResult<()> {
        self.engine.play(&[(at, event)]).map_err(raise)
    }
}

#[pymethods]
impl Engine {
    /// An engine that renders `rate` frames a second when asked, and plays nothing aloud.
    #[staticmethod]
    #[pyo3(signature = (rate = 48000.0))]
    fn offline(rate: f64) -> Engine {
        Engine::with_everything(engine::Engine::offline(rate))
    }

    /// An engine that plays through the output device named, or the default output.
    #[staticmethod]
    #[pyo3(signature = (device = None))]
    fn live(device: Option<String>) -> PyResult<Engine> {
        let settings = Settings {
            device,
            ..Settings::default()
        };
        Ok(Engine::with_everything(
            engine::Engine::live_with(&settings).map_err(raise)?,
        ))
    }

    /// The frames a second the engine runs at.
    #[getter]
    fn rate(&self) -> f64 {
        self.engine.rate()
    }

    /// The engine's clock, in seconds.
    #[getter]
    fn now(&self) -> f64 {
        self.engine.now()
    }

    /// The names of the synths registered, in order.
    fn synths(&self) -> Vec<String> {
        self.engine.synths().map(|s| s.name().to_string()).collect()
    }

    /// Registers the synth in the `mxs-synth` file at `path`, and returns its name.
    fn load_synth(&mut self, path: &str) -> PyResult<String> {
        let synth = engine::graph::synth_from_json_file(path).map_err(raise)?;
        let name = synth.name().to_string();
        self.engine.register(synth);
        Ok(name)
    }

    /// Registers the effect in the `mxs-effect` file at `path`, and returns its name.
    fn load_effect(&mut self, path: &str) -> PyResult<String> {
        let effect = engine::graph::effect_from_json_file(path).map_err(raise)?;
        let name = effect.name().to_string();
        self.engine.register_effect(effect);
        Ok(name)
    }

    /// Loads the sound file at `path`, and returns what a sample event plays it by.
    fn load_sample(&mut self, path: &str) -> PyResult<u64> {
        self.engine.load_sample(path).map_err(raise)
    }

    /// Starts a chain of effects, each a name and a dict of its options, and returns what a note plays into it by.
    fn start_effects(
        &mut self,
        effects: Vec<(String, Option<Bound<'_, PyDict>>)>,
    ) -> PyResult<u64> {
        let effects = effects
            .iter()
            .map(|(name, given)| Ok((name.clone(), options(given.as_ref())?)))
            .collect::<PyResult<Vec<_>>>()?;
        self.engine.start_effects(&effects).map_err(raise)
    }

    /// Plays a note of `synth` at `at` seconds with the options given by name; a held note sounds until released.
    #[pyo3(signature = (at, synth, *, id = None, chain = None, held = false, **given))]
    fn note(
        &mut self,
        at: f64,
        synth: String,
        id: Option<u64>,
        chain: Option<u64>,
        held: bool,
        given: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let options = options(given)?;
        self.play(
            at,
            Event::Note {
                id,
                synth,
                options,
                chain,
                held,
            },
        )
    }

    /// Plays a sample `load_sample` returned at `at` seconds, with the sample player's options given by name.
    #[pyo3(signature = (at, sample, *, id = None, chain = None, **given))]
    fn sample(
        &mut self,
        at: f64,
        sample: u64,
        id: Option<u64>,
        chain: Option<u64>,
        given: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let options = options(given)?;
        self.play(
            at,
            Event::Sample {
                id,
                sample,
                options,
                chain,
            },
        )
    }

    /// Changes options of the sounds started with `ids` at `at` seconds.
    #[pyo3(signature = (at, ids, **given))]
    fn set(&mut self, at: f64, ids: Vec<u64>, given: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let options = numbers(given)?;
        self.play(
            at,
            Event::Set {
                target: Target::Sounds(ids),
                options,
            },
        )
    }

    /// Lets the sounds started with `ids` go at `at` seconds, as keys let up.
    fn release(&mut self, at: f64, ids: Vec<u64>) -> PyResult<()> {
        self.play(at, Event::Release { ids })
    }

    /// Ends the sounds started with `ids` at `at` seconds, fading them over 10 ms.
    fn stop(&mut self, at: f64, ids: Vec<u64>) -> PyResult<()> {
        self.play(at, Event::Stop { ids })
    }

    /// Renders the next `seconds` of an offline engine, as two arrays of samples, left and right.
    fn render<'py>(
        &mut self,
        py: Python<'py>,
        seconds: f64,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let (left, right) = self.engine.render(seconds).map_err(raise)?;
        Ok((array(py, &left)?, array(py, &right)?))
    }

    /// Records the engine's output to the WAV file at `path` from `at` seconds, until `stop_recording_output`.
    #[pyo3(signature = (path, at = 0.0))]
    fn record_output(&mut self, path: &str, at: f64) -> PyResult<()> {
        self.engine.record_output(path, at).map_err(raise)
    }

    /// Stops recording the output, and closes the file.
    fn stop_recording_output(&mut self) -> PyResult<()> {
        self.engine.stop_recording_output().map_err(raise)
    }

    /// Whether anything still sounds.
    fn sounding(&self) -> bool {
        self.engine.sounding()
    }
}

#[pymodule]
fn _mxs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add("Error", m.py().get_type::<Error>())?;
    Ok(())
}
