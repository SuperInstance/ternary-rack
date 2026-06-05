#![forbid(unsafe_code)]

//! Signal routing and patching between ternary rooms.

use std::collections::HashMap;

/// A ternary value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ternary {
    Neg,
    Zero,
    Pos,
}

impl Ternary {
    pub fn to_f64(self) -> f64 {
        match self { Ternary::Neg => -1.0, Ternary::Zero => 0.0, Ternary::Pos => 1.0 }
    }

    pub fn from_f64(v: f64) -> Self {
        if v < -0.33 { Ternary::Neg } else if v > 0.33 { Ternary::Pos } else { Ternary::Zero }
    }
}

/// A processing room: transforms a ternary signal.
pub trait Room: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, input: &[Ternary]) -> Vec<Ternary>;
}

/// A basic room that applies a function to each sample.
pub struct FnRoom {
    name: String,
    f: fn(Ternary) -> Ternary,
}

impl FnRoom {
    pub fn new(name: impl Into<String>, f: fn(Ternary) -> Ternary) -> Self {
        Self { name: name.into(), f }
    }
}

impl Room for FnRoom {
    fn name(&self) -> &str { &self.name }
    fn process(&self, input: &[Ternary]) -> Vec<Ternary> {
        input.iter().map(|&t| (self.f)(t)).collect()
    }
}

/// A room that inverts ternary: Neg↔Pos, Zero stays.
pub fn invert_room(name: &str) -> FnRoom {
    FnRoom::new(name, |t| match t {
        Ternary::Neg => Ternary::Pos,
        Ternary::Zero => Ternary::Zero,
        Ternary::Pos => Ternary::Neg,
    })
}

/// A room that passes through unchanged (identity).
pub fn pass_room(name: &str) -> FnRoom {
    FnRoom::new(name, |t| t)
}

/// A room that zeroes everything.
pub fn zero_room(name: &str) -> FnRoom {
    FnRoom::new(name, |_| Ternary::Zero)
}

// ── Patch cable ────────────────────────────────────────────────────

/// A patch cable connecting one room's output to another room's input.
#[derive(Debug, Clone)]
pub struct PatchCable {
    pub from: String,
    pub to: String,
}

impl PatchCable {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self { from: from.into(), to: to.into() }
    }

    pub fn connect(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::new(from, to)
    }
}

// ── Signal router ──────────────────────────────────────────────────

/// Routes ternary signals through a patch network.
pub struct SignalRouter {
    rooms: HashMap<String, Box<dyn Room>>,
    cables: Vec<PatchCable>,
}

impl SignalRouter {
    pub fn new() -> Self {
        Self { rooms: HashMap::new(), cables: Vec::new() }
    }

    pub fn add_room(&mut self, room: Box<dyn Room>) {
        self.rooms.insert(room.name().to_string(), room);
    }

    pub fn add_cable(&mut self, cable: PatchCable) {
        self.cables.push(cable);
    }

    /// Run signal through the network starting from `input_room`.
    /// Simple approach: follow cables in order, process each room once.
    pub fn route(&self, input: &[Ternary], input_room: &str) -> HashMap<String, Vec<Ternary>> {
        let mut signals: HashMap<String, Vec<Ternary>> = HashMap::new();
        // Seed the input room
        if let Some(room) = self.rooms.get(input_room) {
            signals.insert(input_room.to_string(), room.process(input));
        }
        // Propagate through cables
        for _ in 0..self.cables.len() {
            for cable in &self.cables {
                if let Some(src_signal) = signals.get(&cable.from) {
                    if let Some(room) = self.rooms.get(&cable.to) {
                        let output = room.process(src_signal);
                        signals.entry(cable.to.clone()).or_insert(output);
                    }
                }
            }
        }
        signals
    }
}

// ── Rack ───────────────────────────────────────────────────────────

/// A collection of rooms with patch cables.
pub struct Rack {
    router: SignalRouter,
}

impl Rack {
    pub fn new() -> Self {
        Self { router: SignalRouter::new() }
    }

    pub fn add_room(&mut self, room: Box<dyn Room>) {
        self.router.add_room(room);
    }

    pub fn patch(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.router.add_cable(PatchCable::connect(from, to));
    }

    pub fn run(&self, input: &[Ternary], entry: &str) -> HashMap<String, Vec<Ternary>> {
        self.router.route(input, entry)
    }
}

// ── Serial chain ───────────────────────────────────────────────────

/// Process signal through rooms A → B → C in series.
pub fn serial_chain(rooms: &[&dyn Room], input: &[Ternary]) -> Vec<Ternary> {
    let mut signal = input.to_vec();
    for room in rooms {
        signal = room.process(&signal);
    }
    signal
}

// ── Parallel split ─────────────────────────────────────────────────

/// Split input to multiple rooms simultaneously. Returns each room's output.
pub fn parallel_split(rooms: &[&dyn Room], input: &[Ternary]) -> Vec<Vec<Ternary>> {
    rooms.iter().map(|r| r.process(input)).collect()
}

// ── Feedback loop ──────────────────────────────────────────────────

/// Run A → B, feed B's output back to A's input for `iterations` rounds.
/// The initial input seeds the first iteration.
pub fn feedback_loop(a: &dyn Room, b: &dyn Room, input: &[Ternary], iterations: usize) -> Vec<Ternary> {
    let mut signal = input.to_vec();
    for _ in 0..iterations {
        let b_out = b.process(&a.process(&signal));
        signal = b_out;
    }
    signal
}

// ── Merge bus ──────────────────────────────────────────────────────

/// Merge multiple ternary signals by averaging and re-quantizing.
pub fn merge_bus(signals: &[Vec<Ternary>]) -> Vec<Ternary> {
    if signals.is_empty() { return Vec::new(); }
    let len = signals.iter().map(|s| s.len()).min().unwrap_or(0);
    (0..len).map(|i| {
        let sum: f64 = signals.iter().map(|s| s[i].to_f64()).sum();
        Ternary::from_f64(sum / signals.len() as f64)
    }).collect()
}

// ── Normalize (auto-disconnect on silence) ─────────────────────────

/// Process signal, replacing output with zeros if the input is all zeros.
pub fn normalize_auto_disconnect(input: &[Ternary]) -> Vec<Ternary> {
    let all_zero = input.iter().all(|&t| t == Ternary::Zero);
    if all_zero {
        vec![Ternary::Zero; input.len()]
    } else {
        // Normalize: scale so max magnitude is 1.0 (ternary already is)
        input.to_vec()
    }
}

/// Check if a signal is silent (all zeros).
pub fn is_silent(signal: &[Ternary]) -> bool {
    signal.iter().all(|&t| t == Ternary::Zero)
}

// ════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_signal(n: usize) -> Vec<Ternary> { vec![Ternary::Pos; n] }

    #[test]
    fn test_invert_room() {
        let room = invert_room("inv");
        let out = room.process(&[Ternary::Pos, Ternary::Zero, Ternary::Neg]);
        assert_eq!(out, vec![Ternary::Neg, Ternary::Zero, Ternary::Pos]);
    }

    #[test]
    fn test_pass_room() {
        let room = pass_room("pass");
        let input = vec![Ternary::Pos, Ternary::Neg];
        assert_eq!(room.process(&input), input);
    }

    #[test]
    fn test_zero_room() {
        let room = zero_room("zero");
        let out = room.process(&[Ternary::Pos, Ternary::Neg, Ternary::Zero]);
        assert_eq!(out, vec![Ternary::Zero; 3]);
    }

    #[test]
    fn test_patch_cable() {
        let cable = PatchCable::connect("A", "B");
        assert_eq!(cable.from, "A");
        assert_eq!(cable.to, "B");
    }

    #[test]
    fn test_serial_chain() {
        let inv1 = invert_room("inv1");
        let inv2 = invert_room("inv2");
        let input = vec![Ternary::Pos, Ternary::Neg];
        let out = serial_chain(&[&inv1, &inv2], &input);
        assert_eq!(out, input); // double inversion = identity
    }

    #[test]
    fn test_serial_chain_three() {
        let inv = invert_room("inv");
        let pass = pass_room("pass");
        let input = vec![Ternary::Pos];
        let out = serial_chain(&[&inv, &pass, &inv], &input);
        assert_eq!(out, input);
    }

    #[test]
    fn test_parallel_split() {
        let inv = invert_room("inv");
        let pass = pass_room("pass");
        let input = vec![Ternary::Pos];
        let results = parallel_split(&[&inv, &pass], &input);
        assert_eq!(results[0], vec![Ternary::Neg]);
        assert_eq!(results[1], vec![Ternary::Pos]);
    }

    #[test]
    fn test_feedback_loop() {
        let inv = invert_room("inv");
        let pass = pass_room("pass");
        let input = vec![Ternary::Pos];
        // Even iterations should return to original
        let out = feedback_loop(&inv, &pass, &input, 2);
        assert_eq!(out, vec![Ternary::Pos]);
        // Odd iterations should invert
        let out2 = feedback_loop(&inv, &pass, &input, 1);
        assert_eq!(out2, vec![Ternary::Neg]);
    }

    #[test]
    fn test_merge_bus() {
        let s1 = vec![Ternary::Pos, Ternary::Neg];
        let s2 = vec![Ternary::Neg, Ternary::Pos];
        let merged = merge_bus(&[s1, s2]);
        assert_eq!(merged, vec![Ternary::Zero, Ternary::Zero]);
    }

    #[test]
    fn test_merge_bus_single() {
        let s = vec![Ternary::Pos, Ternary::Neg];
        let merged = merge_bus(&[s.clone()]);
        assert_eq!(merged, s);
    }

    #[test]
    fn test_merge_bus_empty() {
        assert!(merge_bus(&[]).is_empty());
    }

    #[test]
    fn test_normalize_auto_disconnect() {
        let silent = vec![Ternary::Zero; 5];
        assert!(is_silent(&normalize_auto_disconnect(&silent)));
        let active = vec![Ternary::Pos, Ternary::Neg];
        assert!(!is_silent(&normalize_auto_disconnect(&active)));
    }

    #[test]
    fn test_is_silent() {
        assert!(is_silent(&[Ternary::Zero, Ternary::Zero]));
        assert!(!is_silent(&[Ternary::Pos, Ternary::Zero]));
    }

    #[test]
    fn test_rack_basic() {
        let mut rack = Rack::new();
        rack.add_room(Box::new(invert_room("A")));
        rack.add_room(Box::new(invert_room("B")));
        rack.patch("A", "B");
        let results = rack.run(&[Ternary::Pos], "A");
        assert_eq!(results.get("A"), Some(&vec![Ternary::Neg]));
        assert_eq!(results.get("B"), Some(&vec![Ternary::Pos])); // inverted back
    }

    #[test]
    fn test_signal_router() {
        let mut router = SignalRouter::new();
        router.add_room(Box::new(pass_room("X")));
        let results = router.route(&[Ternary::Pos], "X");
        assert_eq!(results.get("X"), Some(&vec![Ternary::Pos]));
    }
}
