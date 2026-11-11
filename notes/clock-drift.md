# The sound card's clock against the system's

cargo run --release --example drift plays silence live for two hours and prints each minute how far the system clock has moved against the sound card's.

On 11 November 2026, on a MacBook Pro's speakers at 44.1 kHz with 13.97 ms of latency, the system clock lost 21.4 ms on the sound card's over 120 minutes, 3.0 parts per million.
It lost 0.7 ms in the first 5 minutes, 7.6 ms by the first hour and 21.4 ms by the second.
Between steps it lost a steady 0.12 to 0.18 ms a minute, 2 to 3 parts per million.
At minutes 6, 38 to 39, 54 to 55, 70 to 71, 90 and 106 it moved at once by 0.3 to 1.4 ms, all but the first in the same direction as the drift.
I'm not sure what caused those steps; the Mac was idle and awake throughout.

So a live engine that took its origin once at start would play an event stamped in system time about 21 ms off after two hours here.
The engine instead lines its clock up with the system's at every output callback, so the drift and the steps are followed as they happen.
