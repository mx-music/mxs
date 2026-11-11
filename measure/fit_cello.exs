# Fits cello.bowed's band gains to the University of Iowa's cello, measuring as mx's designs test does.
# Run from mx: mix run ../mx-engine/measure/fit_cello.exs [rounds], or with score to print scsynth's cello, additive and bowed, beside the engine's.
# The recordings' medians are copied from mx's test/mx/designs_test.exs.

args = System.argv()
System.argv([])
Code.require_file("compare.exs", __DIR__)

defmodule FitCello do
  alias Mx.Measure.Held

  @bands [{80, 160}, {160, 250}, {250, 400}, {400, 630}, {630, 1000}, {1000, 1600}, {1600, 2500}, {2500, 4000}]
  @recorded %{
    0.3 => {0.54, [17.2, 21.6, 17.3, 10.0, 10.9, 0.0, -3.0, -22.9]},
    0.6 => {0.5, [17.6, 21.1, 17.8, 12.2, 15.7, 8.0, 5.2, -13.1]},
    1.0 => {0.39, [15.2, 19.8, 14.3, 12.5, 13.6, 8.9, 7.3, -10.4]}
  }

  def measure(vel, gains, dir) do
    eq = gains |> Enum.with_index() |> Enum.reject(&is_nil(elem(&1, 0))) |> Enum.map(fn {g, i} -> {:"eq#{i}", g} end)

    36..82//3
    |> Task.async_stream(fn note -> Held.note(Compare.engine(:"cello.bowed", [note: note, vel: vel, vibrato: 0, sustain: 2.5] ++ eq, 3, dir), 48_000, note) end, timeout: :infinity)
    |> Enum.map(fn {:ok, n} -> n end)
    |> Held.summary()
  end

  def scsynth(vel, alternates, dir) do
    Mx.Synths.use_alternates(alternates)

    36..82//3
    |> Task.async_stream(fn note -> Held.note(Mx.Render.note(:cello, [note: note, vel: vel, vibrato: 0, sustain: 2.5], 3, dir), 48_000, note) end, timeout: :infinity)
    |> Enum.map(fn {:ok, n} -> n end)
    |> Held.summary()
  end

  def score(vel, dir) do
    {attack, target} = @recorded[vel]
    [today, dwg, rs] = [scsynth(vel, [], dir), scsynth(vel, [:cello], dir), measure(vel, Enum.map(0..7, &Enum.at(gains(vel), &1)), dir)]
    IO.puts("vel #{vel}            rec  today    dwg     rs")
    IO.puts("  attack s     #{Compare.fmt(attack)}#{Compare.fmt(today.attack)}#{Compare.fmt(dwg.attack)}#{Compare.fmt(rs.attack)}")

    for {band, t, a, d, r} <- Enum.zip([@bands, target, levels(today), levels(dwg), levels(rs)]) do
      IO.puts("  #{String.pad_trailing(inspect(band), 13)}#{Compare.fmt(t)}#{Compare.fmt(a)}#{Compare.fmt(d)}#{Compare.fmt(r)}")
    end
  end

  # The engine's own gains, as cello.rs interpolates them.
  defp gains(_vel), do: List.duplicate(nil, 8)

  def levels(summary) do
    measured = Map.new(summary.bands, &{&1.band, &1.level})
    Enum.map(@bands, &measured[&1])
  end

  def fit(vel, rounds, dir) do
    {attack, target} = @recorded[vel]

    gains =
      Enum.reduce(1..rounds//1, List.duplicate(0.0, 8), fn _, gains ->
        levels = levels(measure(vel, gains, dir))
        Enum.zip_with([gains, levels, target], fn [g, l, t] -> g + 0.8 * (t - l) end)
      end)

    summary = measure(vel, gains, dir)
    IO.puts("vel #{vel}: attack rec #{Compare.fmt(attack)} rs #{Compare.fmt(summary.attack)}, vibrato #{inspect(summary.vibrato)}")
    IO.puts("  gains #{Enum.map_join(gains, ", ", &Float.round(&1, 1))}")

    for {band, t, l} <- Enum.zip([@bands, target, levels(summary)]) do
      IO.puts("  #{inspect(band)} rec #{Compare.fmt(t)} rs #{Compare.fmt(l)}")
    end
  end
end

dir = Path.join(System.tmp_dir!(), "mx-engine-compare")
File.mkdir_p!(dir)
case args do
  ["score"] -> for vel <- [0.3, 0.6, 1.0], do: FitCello.score(vel, dir)
  _ -> for vel <- [0.3, 0.6, 1.0], do: FitCello.fit(vel, String.to_integer(List.first(args) || "5"), dir)
end
