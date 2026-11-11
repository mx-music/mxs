# Renders notes through scsynth and through mx-engine, measures both as mx's tests do, and prints them beside the recordings' medians.
# Run from mx: mix run ../mx-engine/measure/compare.exs piano
# The recordings' medians are copied from mx's test/mx/designs_test.exs.

defmodule Compare do
  alias Mx.{Audio, Measure, Render}

  @rate 48_000
  @engine Path.expand("../target/release/render", __DIR__)

  def engine(synth, opts, seconds, dir) do
    wav = Path.join(dir, "#{synth}-#{System.unique_integer([:positive])}.wav")
    args = [to_string(synth), to_string(seconds), wav] ++ Enum.map(opts, fn {k, v} -> "#{k}=#{v / 1}" end)
    {_, 0} = System.cmd(@engine, args, stderr_to_stdout: true)
    {:ok, %{samples: samples, rate: @rate}} = Audio.read(wav)
    File.rm(wav)
    samples
  end

  def both(synth, opts, seconds, dir) do
    {Render.note(synth, opts, seconds, dir), engine(synth, opts, seconds, dir)}
  end

  # The level of the difference between two renders, in dB against the first, each from its first sound: scsynth starts a note 65 samples late.
  def gap(sc, rs) do
    {sc, rs} = {from_sound(sc), from_sound(rs)}
    n = min(length(sc), length(rs))
    {sc, rs} = {Enum.take(sc, n), Enum.take(rs, n)}
    diff = Enum.zip_with(sc, rs, &(&1 - &2))
    db(power(diff) / power(sc))
  end

  def from_sound(samples), do: Enum.drop_while(samples, &(&1 == 0.0))

  def peak_db(samples), do: 20 * :math.log10(Enum.max(Enum.map(samples, &abs/1)) + 1.0e-12)

  defp power(xs), do: Enum.reduce(xs, 0.0, &(&1 * &1 + &2))
  defp db(ratio), do: 10 * :math.log10(ratio + 1.0e-20)

  def fmt(nil), do: "      -"
  def fmt(x) when is_number(x), do: :io_lib.format("~7.1f", [x / 1]) |> to_string()

  def bands_table(title, recorded, sc_notes, rs_notes) do
    sc = Map.new(Measure.bands(sc_notes), &{&1.band, &1})
    rs = Map.new(Measure.bands(rs_notes), &{&1.band, &1})
    IO.puts("\n#{title}\n  band            level dB: rec     sc     rs   | fade s: rec     sc     rs")

    for {band, {level, fade}} <- Enum.sort(recorded) do
      {lo, hi} = band
      s = sc[band] || %{level: nil, fade: nil}
      r = rs[band] || %{level: nil, fade: nil}
      IO.puts("  #{String.pad_trailing("#{lo}-#{hi}", 12)}  #{fmt(level)}#{fmt(s.level)}#{fmt(r.level)}   |  #{fmt(fade)}#{fmt(s.fade)}#{fmt(r.fade)}")
    end
  end

  def run("piano", dir) do
    for {title, recorded, notes, opts} <- [
          {"piano", Recorded.piano(), 36..96//4, []},
          {"piano pedal", Recorded.piano_pedal(), 36..96//4, [pedal: 1]},
          {"piano low", Recorded.piano_low(), [22, 24, 28, 32], []}
        ],
        {vel, table} <- Enum.sort(recorded) do
      notes = Enum.to_list(notes)

      pairs =
        notes
        |> Task.async_stream(&both(:piano, [note: &1, vel: vel, sustain: 60] ++ opts, 8, dir), timeout: :infinity)
        |> Enum.map(fn {:ok, pair} -> pair end)

      sc = Enum.zip_with(notes, pairs, fn n, {s, _} -> Measure.note(s, @rate, n) end)
      rs = Enum.zip_with(notes, pairs, fn n, {_, r} -> Measure.note(r, @rate, n) end)
      gaps = Enum.map(pairs, fn {s, r} -> gap(s, r) end)
      bands_table("#{title} vel #{vel}: difference rs - sc #{fmt(Enum.max(gaps))} dB at worst", table, sc, rs)
    end
  end
end

defmodule CompareBerimbau do
  alias Mx.Measure.Hits
  import Compare, only: [fmt: 1]

  @rate 48_000

  # Measured from EliasArtista's jam, copied from mx's test/mx/designs_test.exs.
  @recorded %{
    {80, 160} => {-22.5, [-23, -23, -26]},
    {160, 250} => {-10.0, [-12, -13, -15]},
    {250, 400} => {-8.2, [-11, -12, -12]},
    {400, 630} => {-10.3, [-16, -18, -16]},
    {630, 1000} => {-11.7, [-15, -16, -18]},
    {1000, 1600} => {-10.1, [-13, -14, -16]},
    {1600, 2500} => {-9.9, [-12, -14, -18]},
    {2500, 4000} => {0.0, [-6, -12, -19]},
    {4000, 6300} => {-0.3, [-12, -20, -22]},
    {6300, 10_000} => {-5.0, [-19, -27, -27]},
    {10_000, 16_000} => {-9.6, [-26, -32, -33]}
  }

  def run(dir) do
    both = fn opts, seconds -> Compare.both(:berimbau, opts, seconds, dir) end
    {sc, rs} = both.([gourd: 0], 3)
    [scs, rss] = for s <- [sc, rs], do: Hits.hit(Compare.from_sound(s), @rate, nil)
    level = fn hit -> Map.new(Hits.summary([hit]), &{&1.band, &1.level}) end
    later = fn hit -> Map.new(Hits.moments([hit]), &{&1.band, tl(&1.levels)}) end
    [scl, rsl, scm, rsm] = [level.(scs), level.(rss), later.(scs), later.(rss)]
    IO.puts("\nberimbau gourd 0, rms 0-0.5 s rs - sc #{fmt(db(rms(rs, 0, 0.5) / rms(sc, 0, 0.5)))} dB")
    IO.puts("  band           at strike: rec     sc     rs   | 50, 100, 200 ms later: rec / sc / rs")

    for {band, {l, ls}} <- Enum.sort(@recorded) do
      {lo, hi} = band
      row = fn xs -> Enum.map_join(xs, "", &fmt/1) end
      IO.puts("  #{String.pad_trailing("#{lo}-#{hi}", 12)}  #{fmt(l)}#{fmt(scl[band])}#{fmt(rsl[band])}   | #{row.(ls)} /#{row.(scm[band])} /#{row.(rsm[band])}")
    end

    for {opts, string} <- [{[], 78.28}, {[coin: 1], 83.14}, {[note: 41], 87.31}] do
      {sc, rs} = both.(opts, 3)
      worst = fn s -> Hits.hit(s, @rate, nil, partials: 12).partials |> Enum.map(&abs(&1.hz / (round(&1.hz / string) * string) - 1)) |> Enum.max() end
      IO.puts("partials #{inspect(opts)}: worst off a harmonic of #{string} Hz, sc #{Float.round(worst.(sc) * 100, 1)}% rs #{Float.round(worst.(rs) * 100, 1)}% (test: 3%)")
    end

    {sc, rs} = both.([], 3)
    strongest = fn s -> Hits.hit(s, @rate, nil, partials: 12).partials |> Enum.sort_by(& &1.level, :desc) |> Enum.take(2) |> Enum.map(&round(&1.hz)) |> Enum.sort() end
    IO.puts("strongest two partials (test: 233 and 390 Hz): sc #{inspect(strongest.(sc))} rs #{inspect(strongest.(rs))}")

    for opts <- [[], [coin: 1], [buzz: 1]] do
      {scs, rss} = both.([vel: 0.3] ++ opts, 1)
      {sch, rsh} = both.([vel: 1] ++ opts, 1)
      IO.puts("vel 1 over 0.3 #{inspect(opts)} (test: > 6 dB): sc #{fmt(db(rms(sch, 0, 0.2) / rms(scs, 0, 0.2)))} rs #{fmt(db(rms(rsh, 0, 0.2) / rms(rss, 0, 0.2)))}")
    end

    for opts <- [[], [gourd: 0]] do
      {sc, rs} = both.(opts, 2.2)
      IO.puts("gourd #{inspect(opts)}: 5th against 3rd swings sc #{swing(sc)} rs #{swing(rs)} (test: > 20 dB with peaks 0.43 s apart, < 6 dB still)")
    end

    {sco, rso} = both.([], 1)
    {scb, rsb} = both.([buzz: 1], 1)
    for {name, open, buzz} <- [{"sc", sco, scb}, {"rs", rso, rsb}] do
      IO.puts("buzz #{name}: 0.2-0.3 s against 0-0.05 s #{fmt(db(rms(buzz, 0.2, 0.3) / rms(buzz, 0, 0.05)))} dB (test: < -29.5), open #{fmt(db(rms(open, 0.2, 0.3) / rms(open, 0, 0.05)))} dB (test: > -14)")
    end
  end

  defp swing(sound) do
    ratios =
      for at <- Enum.map(2..100, &(&1 * 0.02)) do
        samples = window(sound, at, at + 0.09)
        10 * :math.log10(goertzel(samples, 5 * 78.28) / goertzel(samples, 3 * 78.28))
      end

    peaks =
      for {[before, here, next], index} <- Enum.with_index(Enum.chunk_every(ratios, 3, 1, :discard)),
          here > before and here >= next and here > Enum.max(ratios) - 10,
          do: (index + 3) * 0.02

    gaps = Enum.zip_with(Enum.drop(peaks, 1), peaks, &(&1 - &2))
    median = if gaps == [], do: nil, else: Enum.at(Enum.sort(gaps), div(length(gaps), 2))
    "#{fmt(Enum.max(ratios) - Enum.min(ratios))} dB, peaks #{if median, do: Float.round(median, 2), else: "-"} s apart"
  end

  defp window(sound, from, to), do: Enum.slice(sound, round(from * @rate), round((to - from) * @rate))

  defp goertzel(samples, hz) do
    w = 2 * :math.pi() * hz / @rate
    {s1, s2} = Enum.reduce(samples, {0.0, 0.0}, fn x, {s1, s2} -> {x + 2 * :math.cos(w) * s1 - s2, s1} end)
    s1 * s1 + s2 * s2 - 2 * :math.cos(w) * s1 * s2
  end

  defp rms(sound, from, to) do
    xs = window(sound, from, to)
    :math.sqrt(Enum.reduce(xs, 0.0, &(&1 * &1 + &2)) / length(xs))
  end

  defp db(ratio), do: 20 * :math.log10(ratio)
end

defmodule Recorded do
  def piano do
    %{
      0.3 => %{{80, 160} => {6.6, 9.37}, {160, 250} => {5.3, 11.87}, {250, 400} => {-0.2, 10.48}, {400, 630} => {-2.9, 12.41}, {630, 1000} => {-8.6, 11.21}, {1000, 1600} => {-25.3, nil}, {1600, 2500} => {-41.6, nil}, {2500, 4000} => {-54.5, nil}},
      0.6 => %{{80, 160} => {4.9, 8.42}, {160, 250} => {6.4, 11.07}, {250, 400} => {1.1, 10.79}, {400, 630} => {0.1, 10.79}, {630, 1000} => {-3.4, 10.25}, {1000, 1600} => {-10.0, 10.15}, {1600, 2500} => {-20.7, 7.96}, {2500, 4000} => {-29.8, 5.32}},
      1.0 => %{{80, 160} => {5.7, 8.45}, {160, 250} => {6.4, 10.39}, {250, 400} => {0.9, 10.68}, {400, 630} => {-0.3, 9.84}, {630, 1000} => {-3.4, 10.43}, {1000, 1600} => {-7.4, 8.72}, {1600, 2500} => {-16.3, 6.69}, {2500, 4000} => {-20.9, 4.43}}
    }
  end

  def piano_pedal do
    %{
      0.3 => %{{80, 160} => {6.5, 15.10}, {160, 250} => {5.7, 14.73}, {250, 400} => {3.1, 12.81}, {400, 630} => {-0.9, 11.45}, {630, 1000} => {-3.5, 11.65}, {1000, 1600} => {-10.2, 10.24}, {1600, 2500} => {-17.9, 7.55}, {2500, 4000} => {-27.9, 5.91}},
      0.6 => %{{80, 160} => {5.9, 15.81}, {160, 250} => {5.3, 14.82}, {250, 400} => {2.3, 13.78}, {400, 630} => {-0.4, 11.68}, {630, 1000} => {-3.4, 10.21}, {1000, 1600} => {-9.4, 10.07}, {1600, 2500} => {-14.9, 7.84}, {2500, 4000} => {-22.5, 5.06}},
      1.0 => %{{80, 160} => {6.1, 14.90}, {160, 250} => {5.5, 13.11}, {250, 400} => {1.7, 15.68}, {400, 630} => {-1.1, 11.56}, {630, 1000} => {-2.8, 10.46}, {1000, 1600} => {-8.7, 9.67}, {1600, 2500} => {-15.7, 7.20}, {2500, 4000} => {-20.0, 4.71}}
    }
  end

  def piano_low do
    %{
      0.3 => %{{80, 160} => {19.4, 15.74}, {160, 250} => {13.9, 14.59}, {250, 400} => {7.1, 9.57}, {400, 630} => {-2.5, 17.02}, {630, 1000} => {-3.7, 15.30}, {1000, 1600} => {-7.4, 20.75}},
      0.6 => %{{80, 160} => {28.0, 14.71}, {160, 250} => {25.0, 11.17}, {250, 400} => {19.4, 8.54}, {400, 630} => {15.5, 12.50}, {630, 1000} => {16.9, 14.09}, {1000, 1600} => {15.6, 11.48}},
      1.0 => %{{80, 160} => {33.0, 15.57}, {160, 250} => {30.9, 11.80}, {250, 400} => {25.9, 8.40}, {400, 630} => {21.3, 11.76}, {630, 1000} => {22.6, 13.33}, {1000, 1600} => {nil, 10.63}}
    }
  end
end

dir = Path.join(System.tmp_dir!(), "mx-engine-compare")
File.mkdir_p!(dir)
for synth <- System.argv() do
  if synth == "berimbau", do: CompareBerimbau.run(dir), else: Compare.run(synth, dir)
end
