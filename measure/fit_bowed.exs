# Fits the band gains of mx's bowed strings to the University of Iowa's recordings, measuring as mx's designs test does.
# Run from mx: mix run ../mxs/measure/fit_bowed.exs violin [rounds], or with score in place of rounds to print the fitted figures.
# The recordings' medians, and the notes each is measured over, are copied from mx's test/mx/designs_test.exs.

args = System.argv()
System.argv([])

defmodule FitBowed do
  alias Mx.{Audio, Measure.Held}

  @render Path.expand("../target/release/render", __DIR__)
  @bands [{80, 160}, {160, 250}, {250, 400}, {400, 630}, {630, 1000}, {1000, 1600}, {1600, 2500}, {2500, 4000}]
  @recorded %{
    violin: {55..107//4, %{
      0.3 => {0.75, %{{160, 250} => 4.7, {250, 400} => 21.9, {400, 630} => 13.9, {630, 1000} => 11.0, {1000, 1600} => 0.8, {1600, 2500} => 2.8, {2500, 4000} => -8.4}},
      0.6 => {0.61, %{{160, 250} => 5.2, {250, 400} => 20.5, {400, 630} => 19.5, {630, 1000} => 14.8, {1000, 1600} => 11.7, {1600, 2500} => 11.7, {2500, 4000} => 2.8}},
      1.0 => {0.3, %{{160, 250} => 1.6, {250, 400} => 18.4, {400, 630} => 19.3, {630, 1000} => 16.3, {1000, 1600} => 11.0, {1600, 2500} => 11.3, {2500, 4000} => 6.1}}
    }},
    viola: {48..82//3, %{
      0.3 => {0.53, %{{80, 160} => 1.8, {160, 250} => 22.3, {250, 400} => 30.0, {400, 630} => 26.6, {630, 1000} => 26.9, {1000, 1600} => 21.6, {1600, 2500} => 18.8, {2500, 4000} => 0.2}},
      0.6 => {0.55, %{{80, 160} => 0.4, {160, 250} => 23.9, {250, 400} => 29.3, {400, 630} => 27.6, {630, 1000} => 29.2, {1000, 1600} => 23.4, {1600, 2500} => 21.7, {2500, 4000} => 5.9}},
      1.0 => {0.32, %{{80, 160} => 0.2, {160, 250} => 24.8, {250, 400} => 32.1, {400, 630} => 27.4, {630, 1000} => 28.2, {1000, 1600} => 25.8, {1600, 2500} => 24.2, {2500, 4000} => 8.0}}
    }},
    cello: {36..82//3, %{
      0.3 => {0.54, %{{80, 160} => 17.2, {160, 250} => 21.6, {250, 400} => 17.3, {400, 630} => 10.0, {630, 1000} => 10.9, {1000, 1600} => 0.0, {1600, 2500} => -3.0, {2500, 4000} => -22.9}},
      0.6 => {0.5, %{{80, 160} => 17.6, {160, 250} => 21.1, {250, 400} => 17.8, {400, 630} => 12.2, {630, 1000} => 15.7, {1000, 1600} => 8.0, {1600, 2500} => 5.2, {2500, 4000} => -13.1}},
      1.0 => {0.39, %{{80, 160} => 15.2, {160, 250} => 19.8, {250, 400} => 14.3, {400, 630} => 12.5, {630, 1000} => 13.6, {1000, 1600} => 8.9, {1600, 2500} => 7.3, {2500, 4000} => -10.4}}
    }},
    double_bass: {28..67//3, %{
      0.3 => {0.28, %{{80, 160} => 21.0, {160, 250} => 12.0, {250, 400} => -0.5, {400, 630} => -8.9, {630, 1000} => -15.5, {1000, 1600} => -19.7, {1600, 2500} => -26.4, {2500, 4000} => -35.7}},
      0.6 => {0.29, %{{80, 160} => 23.6, {160, 250} => 15.1, {250, 400} => 4.4, {400, 630} => -3.6, {630, 1000} => -11.6, {1000, 1600} => -15.5, {1600, 2500} => -22.6, {2500, 4000} => -34.0}},
      1.0 => {0.16, %{{80, 160} => 27.7, {160, 250} => 20.7, {250, 400} => 10.3, {400, 630} => 4.1, {630, 1000} => -3.0, {1000, 1600} => -7.7, {1600, 2500} => -13.9, {2500, 4000} => -24.9}}
    }}
  }

  def render(synth, opts, seconds, dir) do
    wav = Path.join(dir, "#{synth}-#{System.unique_integer([:positive])}.wav")
    args = [to_string(synth), to_string(seconds), wav] ++ Enum.map(opts, fn {k, v} -> "#{k}=#{v / 1}" end)
    {_, 0} = System.cmd(@render, args, stderr_to_stdout: true)
    {:ok, %{samples: samples}} = Audio.read(wav)
    File.rm(wav)
    samples
  end

  def measure(synth, vel, gains, dir, extra \\ []) do
    {notes, _} = @recorded[synth]
    eq = (gains |> Enum.with_index() |> Enum.reject(&is_nil(elem(&1, 0))) |> Enum.map(fn {g, i} -> {:"eq#{i}", g} end)) ++ extra

    notes
    |> Task.async_stream(fn note -> Held.note(render(synth, [note: note, vel: vel, vibrato: 0, sustain: 2.5] ++ eq, 3, dir), 48_000, note) end, timeout: :infinity)
    |> Enum.map(fn {:ok, n} -> n end)
    |> Held.summary()
  end

  def levels(summary) do
    measured = Map.new(summary.bands, &{&1.band, &1.level})
    Enum.map(@bands, &measured[&1])
  end

  def targets(synth, vel) do
    {_, by_vel} = @recorded[synth]
    {attack, bands} = by_vel[vel]
    {attack, Enum.map(@bands, &bands[&1])}
  end

  # Moves each band's gain most of the way to its target each round.
  # A band the recordings do not measure keeps 0, and so does the band holding the lowest note's fundamental, which every band is measured against.
  def fit(synth, vel, rounds, dir) do
    {_, target} = targets(synth, vel)
    {notes, _} = @recorded[synth]
    lowest = 440 * :math.pow(2, (Enum.min(notes) - 69) / 12)
    reference = Enum.find_index(@bands, fn {low, high} -> lowest >= low and lowest < high end)

    gains =
      Enum.reduce(1..rounds//1, List.duplicate(0.0, 8), fn _, gains ->
        levels = levels(measure(synth, vel, gains, dir))

        Enum.zip_with([gains, levels, target, Enum.to_list(0..7)], fn
          [g, _, _, ^reference] -> g
          [g, l, t, _] when is_number(l) and is_number(t) -> g + 0.6 * (t - l)
          [g, _, _, _] -> g
        end)
      end)

    report(synth, vel, measure(synth, vel, gains, dir))
    IO.puts("  gains #{Enum.map_join(gains, ", ", &Float.round(&1, 1))}")
  end

  # Each body corner's misses against the recordings with no gains, and their spread, which gains then have to make up.
  def body(synth, dir) do
    {notes, _} = @recorded[synth]
    lowest = 440 * :math.pow(2, (Enum.min(notes) - 69) / 12)

    for factor <- [0.75, 1.0, 1.25, 1.5, 2.0] do
      misses =
        for vel <- [0.3, 0.6, 1.0] do
          {_, target} = targets(synth, vel)
          levels = levels(measure(synth, vel, List.duplicate(nil, 8), dir, body_hz: lowest * factor))
          for [l, t] <- Enum.zip_with(levels, target, &[&1, &2]), is_number(l) and is_number(t), do: t - l
        end

      all = List.flatten(misses)
      mean = Enum.sum(all) / length(all)
      spread = :math.sqrt(Enum.sum(Enum.map(all, &((&1 - mean) ** 2))) / length(all))
      IO.puts("#{synth} body #{Float.round(lowest * factor, 1)} Hz: spread #{Float.round(spread, 1)} dB, misses #{inspect(Enum.map(misses, fn m -> Enum.map(m, &Float.round(&1, 0)) end))}")
    end
  end

  def report(synth, vel, summary) do
    {attack, target} = targets(synth, vel)
    IO.puts("#{synth} vel #{vel}: attack rec #{fmt(attack)} engine #{fmt(summary.attack)}")

    for {band, t, l} <- Enum.zip([@bands, target, levels(summary)]), t != nil do
      mark = if is_number(l) and abs(l - t) > 6, do: " <--", else: ""
      IO.puts("  #{String.pad_trailing(inspect(band), 13)} rec #{fmt(t)} engine #{fmt(l)}#{mark}")
    end
  end

  defp fmt(nil), do: "      -"
  defp fmt(x), do: x |> Float.round(2) |> to_string() |> String.pad_leading(7)
end

if args != [] do
  dir = Path.join(System.tmp_dir!(), "mxs-fit-bowed")
  File.mkdir_p!(dir)
  [synth | rest] = args
  synth = String.to_atom(synth)

  case rest do
    ["body"] -> FitBowed.body(synth, dir)
    ["score"] -> for vel <- [0.3, 0.6, 1.0], do: FitBowed.report(synth, vel, FitBowed.measure(synth, vel, List.duplicate(nil, 8), dir))
    _ -> for vel <- [0.3, 0.6, 1.0], do: FitBowed.fit(synth, vel, String.to_integer(List.first(rest) || "5"), dir)
  end
end
