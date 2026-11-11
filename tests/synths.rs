use mxs::{Engine, Event, RATE, synths};

#[test]
fn every_synth_renders_a_finite_audible_note_with_its_defaults() {
    for synth in synths::all() {
        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        engine.play(&[(0.0, Event::note(None, synth.name(), &[]))]).unwrap();
        let (left, right) = engine.render(1.0).unwrap();
        let name = synth.name();
        assert!(left.iter().chain(&right).all(|x| x.is_finite()), "{name} gave a sample that is not finite");
        let peak = left.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 1e-4, "{name} is silent");
        assert!(peak < 4.0, "{name} peaks at {peak}");
    }
}
