use mapper_core::{process_stick, StickConfig};

fn main() {
    let cfg = StickConfig { inner_deadzone: 0.12, outer_deadzone: 0.95, curve_exponent: 1.5 };
    let output = process_stick(0.5, 0.5, &cfg);
    println!("Input Mapper Ready! Processed Stick Output: {:?}", output);
}
