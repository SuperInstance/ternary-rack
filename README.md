# ternary-rack

**Patch any agent into any other.** The pedalboard/rack metaphor — modular signal routing for ternary processing chains.

## Why This Exists

In a complex multi-agent system, you need to compose processing stages. An agent's output might need to be inverted, filtered, mixed with another agent's signal, fed back through a chain, and then routed to a final output. Hardcoding these paths is brittle.

Audio engineers solved this with patch cables and rack-mounted effects units. Each unit does one thing. You connect them with cables in any configuration: serial chains, parallel splits, feedback loops. The same unit can appear in multiple chains.

This crate implements that model for ternary signals. A **Room** is a processing unit (like an effects pedal). A **PatchCable** connects one room's output to another's input. A **Rack** holds rooms and cables, and routes signals through the network. You can build serial chains, parallel splits, feedback loops, and merge buses.

## The Architecture

### Rooms

A Room is anything that transforms a ternary signal:

```rust
pub trait Room: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, input: &[Ternary]) -> Vec<Ternary>;
}
```

Built-in rooms:

- `invert_room` — Neg↔Pos, Zero stays. The contrarianizer: turns agreement into disagreement and vice versa.
- `pass_room` — identity transform. Useful as a tap point or merge target.
- `zero_room` — silences everything. The ultimate mute.
- `FnRoom` — wrap any `fn(Ternary) -> Ternary` as a room.

### Patch Cables

Cables connect rooms by name. A cable from "A" to "B" means: take room A's output and feed it to room B's input. The routing engine follows cables in order, propagating signals through the network.

```rust
let cable = PatchCable::connect("compressor", "reverb");
```

### Serial Chains

Process signal through rooms in sequence: A → B → C. Each room receives the output of the previous one. This is the standard effects chain model.

```rust
let result = serial_chain(&[&inv1, &inv2], &input);
// Double inversion = identity
```

### Parallel Splits

Send the same input to multiple rooms simultaneously. Each room produces its own output. This is the aux-send model: one signal, multiple parallel processing paths.

```rust
let results = parallel_split(&[&inv, &pass], &input);
// results[0] = inverted, results[1] = original
```

### Feedback Loops

Run A → B, then feed B's output back to A's input for N iterations. This is how oscillation, resonance, and iterative convergence work. In agent dynamics, a feedback loop models agents that keep reacting to each other's reactions.

```rust
// Even iterations of invert → pass return to original
let out = feedback_loop(&inv, &pass, &input, 2);
```

### Merge Bus

Sum multiple ternary signals by averaging and re-quantizing. Two opposing signals (Pos + Neg) cancel to Zero. Two agreeing signals (Pos + Pos) reinforce to Pos. This is the voting mechanism.

```rust
let merged = merge_bus(&[vec![Ternary::Pos], vec![Ternary::Neg]]);
// Result: [Zero] — perfect cancellation
```

## Key Types and Functions

```rust
/// A ternary value.
pub enum Ternary { Neg, Zero, Pos }

/// A processing room: transforms a ternary signal.
pub trait Room: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, input: &[Ternary]) -> Vec<Ternary>;
}

/// A room that applies a function to each sample.
pub struct FnRoom { /* ... */ }
impl FnRoom {
    pub fn new(name: impl Into<String>, f: fn(Ternary) -> Ternary) -> Self
}

/// Built-in rooms.
pub fn invert_room(name: &str) -> FnRoom  // Neg↔Pos
pub fn pass_room(name: &str) -> FnRoom    // identity
pub fn zero_room(name: &str) -> FnRoom    // silence

/// A patch cable connecting rooms.
pub struct PatchCable { pub from: String, pub to: String }
impl PatchCable {
    pub fn connect(from: impl Into<String>, to: impl Into<String>) -> Self
}

/// Routes signals through a named room network.
pub struct SignalRouter { /* ... */ }
impl SignalRouter {
    pub fn new() -> Self
    pub fn add_room(&mut self, room: Box<dyn Room>)
    pub fn add_cable(&mut self, cable: PatchCable)
    pub fn route(&self, input: &[Ternary], input_room: &str) -> HashMap<String, Vec<Ternary>>
}

/// Top-level rack holding rooms and cables.
pub struct Rack { /* ... */ }
impl Rack {
    pub fn new() -> Self
    pub fn add_room(&mut self, room: Box<dyn Room>)
    pub fn patch(&mut self, from: impl Into<String>, to: impl Into<String>)
    pub fn run(&self, input: &[Ternary], entry: &str) -> HashMap<String, Vec<Ternary>>
}

/// Serial chain: A → B → C.
pub fn serial_chain(rooms: &[&dyn Room], input: &[Ternary]) -> Vec<Ternary>

/// Parallel split: input goes to all rooms simultaneously.
pub fn parallel_split(rooms: &[&dyn Room], input: &[Ternary]) -> Vec<Vec<Ternary>>

/// Feedback loop: A → B → A → B → ... for N iterations.
pub fn feedback_loop(a: &dyn Room, b: &dyn Room, input: &[Ternary], iterations: usize) -> Vec<Ternary>

/// Merge multiple signals by averaging and re-quantizing.
pub fn merge_bus(signals: &[Vec<Ternary>]) -> Vec<Ternary>
```

## Usage

### Build a Rack

```rust
use ternary_rack::{Rack, invert_room, pass_room, zero_room};

let mut rack = Rack::new();

// Add processing rooms
rack.add_room(Box::new(invert_room("contrarianizer")));
rack.add_room(Box::new(pass_room("monitor")));
rack.add_room(Box::new(invert_room("double_neg")));

// Patch: input → contrarianizer → monitor
//         contrarianizer → double_neg (inverts back)
rack.patch("contrarianizer", "monitor");
rack.patch("contrarianizer", "double_neg");

let input = vec![Ternary::Pos, Ternary::Neg, Ternary::Zero];
let results = rack.run(&input, "contrarianizer");

// results["contrarianizer"] = [Neg, Pos, Zero]
// results["monitor"]        = [Neg, Pos, Zero]  (passed through)
// results["double_neg"]     = [Pos, Neg, Zero]  (inverted back)
```

### Serial Chain

```rust
use ternary_rack::{serial_chain, invert_room};

let inv1 = invert_room("inv1");
let inv2 = invert_room("inv2");
let input = vec![Ternary::Pos, Ternary::Neg];

let result = serial_chain(&[&inv1, &inv2], &input);
// Double inversion = [Pos, Neg] — back to original
```

### Feedback Loop

```rust
use ternary_rack::{feedback_loop, invert_room, pass_room};

let inv = invert_room("inv");
let pass = pass_room("pass");
let input = vec![Ternary::Pos];

let even = feedback_loop(&inv, &pass, &input, 2);  // [Pos] — stable
let odd  = feedback_loop(&inv, &pass, &input, 1);  // [Neg] — flipped
```

### Custom Rooms

```rust
use ternary_rack::{FnRoom, Room};

// A room that forces everything to +1 (agreeable)
let force_agree = FnRoom::new("agreeable", |t| Ternary::Pos);

// A room that collapses to nearest extreme (no middle ground)
let radicalize = FnRoom::new("radical", |t| match t {
    Ternary::Zero => Ternary::Pos,  // fence-sitters become agreeable
    other => other,
});
```

## The 8-Ball Connection

`normalize_auto_disconnect` and `is_silent` handle the edge case where a room receives all zeros — a silent signal. In the rack model, silence propagation is important: if a room goes silent, you want to detect it and potentially disconnect the cable (auto-disconnect) rather than processing silence through the entire chain.

## In the Ternary Fleet

This is the **routing infrastructure** in the DJ metaphor product stack:

- `ternary-mixer` — provides the channel strips that feed into the rack
- **ternary-rack** — routes and patches processing chains
- `ternary-envelope` — rooms that shape ADSR dynamics
- `ternary-crossfader` — a room that blends two inputs
- `ternary-grain` — a room that granulates and recombines

Any processing step can be wrapped as a `Room` and patched into the rack.

## License

MIT

## See Also
- **ternary-wave** — related
- **ternary-bite** — related
- **ternary-envelope** — related
- **ternary-echo** — related
- **ternary-pan** — related
- **ternary-mixer** — related
- **ternary-vu** — related
- **ternary-gate** — related
- **ternary-sampler** — related

